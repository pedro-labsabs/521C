import { create } from "zustand";
import { CHAR, DESTRUCTIVE_CMDS, encodeKeyFunctionDirect, set } from "./protocol";
import { WriteDeniedError } from "./policy";
import {
  CONFIG_SCHEMA_VERSION,
  buildExport,
  loadPersistedConfig,
  parseImport,
  savePersistedConfig,
  summarizeErrors,
  type ExternalConfig,
  type ParseResult,
  type PersistedConfig,
} from "./config-schema";
import {
  CHIME_COOLDOWN_MS,
  chimeToneId,
  evaluateChimePreflight,
  type ChimePreflight,
  type ChimeSide,
} from "./find-preflight";
import { CommandScheduler, type CommandResult } from "./scheduler";
import type { AncScene, KeyBinding } from "./protocol/types";
import { DEVICE_EQ_PRESETS, type NamedEq, presetFromGains } from "./eq-presets";
import { BUILTIN_PROFILES, type NoiseUiMode, type SmartProfile } from "./smart-profiles";
import {
  MockTransport,
  WebBluetoothTransport,
  createInitialState,
  webBluetoothAvailable,
  type DeviceLiveState,
  type DiscoveredDevice,
  type PacketLogEntry,
  type QcyTransport,
  type TransportKind,
} from "./transport";

export type HubView =
  | "overview"
  | "noise"
  | "sound"
  | "controls"
  | "profiles"
  | "device"
  | "advanced"
  | "cli"
  | "developer";

// Theme/notification types now live in the versioned config schema (issue #11);
// re-exported so existing imports keep working.
export type { ThemeMode, NotifyPrefs } from "./config-schema";
import type { NotifyPrefs, ThemeMode } from "./config-schema";

const browserStorage: import("./config-schema").ConfigStorage | undefined =
  typeof localStorage === "undefined" ? undefined : localStorage;

/**
 * Load + validate + migrate persisted config (issue #11). Corrupt or newer payloads
 * fall back to defaults without destroying stored data.
 */
const persistedLoad = loadPersistedConfig(browserStorage);
const persisted: PersistedConfig = persistedLoad.config;

export type HubState = {
  view: HubView;
  theme: ThemeMode;
  transportKind: TransportKind;
  scanning: boolean;
  discovered: DiscoveredDevice[];
  device: DeviceLiveState;
  log: PacketLogEntry[];
  toast: { id: number; title: string; body: string } | null;
  notify: NotifyPrefs;
  hideMac: boolean;
  customEq: NamedEq[];
  customProfiles: SmartProfile[];
  activeProfileId: string;
  autoGame: boolean;
  autoGameKeyword: string;
  sleepTimerMin: number;
  eqAb: { a: number[]; b: number[]; using: "a" | "b" } | null;
  systemEqOn: boolean;
  systemEqGains: number[];
  cliHistory: string[];
  minimized: boolean;
  /** Session-scoped opt-in for experimental device writes. Never persisted. */
  experimentalOptIn: boolean;
  /** Pending Find-Earbuds chime awaiting interactive confirmation. No TX until confirmed. */
  pendingChime: ChimePreflight | null;
  /** Epoch ms before which another chime is refused (rate limit). */
  chimeBlockedUntil: number;
};

/** Structured per-step outcome of applying a smart profile (issue #10). */
export type ProfileStepResult = { step: string; ok: boolean };
export type ProfileApplyResult = {
  profileId: string;
  profileName: string;
  steps: ProfileStepResult[];
  ok: boolean;
  /** Observed device state after application, for reconciliation. */
  observed: { noiseMode: number; gameMode: boolean; inEar: boolean; eqName: string };
};

export type HubActions = {
  setView: (view: HubView) => void;
  setTheme: (theme: ThemeMode) => void;
  scan: () => Promise<void>;
  connect: (id: string, kind?: TransportKind) => Promise<void>;
  /** Explicit user-gesture flow to connect a real device over Web Bluetooth (issue #2). */
  connectWebBluetooth: () => Promise<void>;
  /** Explicitly (re)select the mock transport. */
  connectMock: () => Promise<void>;
  disconnect: () => Promise<void>;
  setNoise: (mode: NoiseUiMode, level?: number) => Promise<boolean>;
  setGameMode: (on: boolean) => Promise<boolean>;
  setSleep: (on: boolean) => Promise<void>;
  setSpatial: (on: boolean) => Promise<void>;
  setInEar: (on: boolean) => Promise<boolean>;
  setWear: (partial: Partial<DeviceLiveState["wear"]>) => Promise<void>;
  setEq: (named: NamedEq) => Promise<boolean>;
  setEqGains: (gains: number[], name?: string) => Promise<void>;
  setBinding: (keyId: number, funId: number) => Promise<void>;
  setSoundBalance: (value: number) => Promise<void>;
  /** Compute the chime preflight and stage it for confirmation. Never transmits. */
  requestChime: (side: ChimeSide) => void;
  /** Dismiss a staged chime without transmitting. */
  cancelChime: () => void;
  /** Transmit the staged chime only after confirmation passes all safety gates. */
  confirmChime: () => Promise<void>;
  applyProfile: (id: string) => Promise<ProfileApplyResult | null>;
  saveCustomProfile: (profile: SmartProfile) => void;
  media: (action: "play" | "pause" | "prev" | "next") => Promise<void>;
  runCli: (line: string) => Promise<string>;
  exportConfig: () => string;
  /**
   * Validate and apply an imported config atomically (issue #11). Returns structured
   * errors without touching state when the file is invalid.
   */
  importConfig: (json: string) => ParseResult<ExternalConfig>;
  exportDiagnostics: () => string;
  setNotify: (partial: Partial<NotifyPrefs>) => void;
  setHideMac: (hide: boolean) => void;
  setMinimized: (v: boolean) => void;
  setAutoGame: (on: boolean, keyword?: string) => void;
  setSystemEq: (on: boolean, gains?: number[]) => void;
  toggleEqAb: () => void;
  snapshotEqAb: (slot: "a" | "b") => void;
  setWorn: (left: boolean, right: boolean) => void;
  clearLog: () => void;
  dismissToast: () => void;
  setExperimentalOptIn: (on: boolean) => void;
};

let transport: QcyTransport = new MockTransport();
/**
 * Per-connection command scheduler (issue #10). Serializes device writes and coalesces
 * high-frequency latest-value controls. Recreated on connect; queued work is cancelled
 * on disconnect.
 */
let scheduler = new CommandScheduler();
let writeSeq = 0;
let toastSeq = 1;

function persistFrom(get: () => HubState) {
  const s = get();
  savePersistedConfig(browserStorage, {
    schema: CONFIG_SCHEMA_VERSION,
    theme: s.theme,
    notify: s.notify,
    hideMac: s.hideMac,
    customEq: s.customEq,
    customProfiles: s.customProfiles,
    activeProfileId: s.activeProfileId,
    autoGame: s.autoGame,
    autoGameKeyword: s.autoGameKeyword,
    sleepTimerMin: s.sleepTimerMin,
    lastSeen: s.device.lastSeen,
  });
}

function noiseToScene(mode: NoiseUiMode, level: number): { scene: AncScene; simple: number; adaptive: boolean } {
  const lv = Math.max(1, Math.min(3, level));
  switch (mode) {
    case "off":
      return { scene: { mode: 0x00, subScene: 0x00, noiseValue: 0 }, simple: 0x00, adaptive: false };
    case "anc":
      return { scene: { mode: 0x02, subScene: lv, noiseValue: 80 }, simple: 0x01, adaptive: false };
    case "adaptive":
      return { scene: { mode: 0x02, subScene: lv, noiseValue: 80 }, simple: 0x01, adaptive: true };
    case "indoor":
      return { scene: { mode: 0x02, subScene: lv, noiseValue: 70 }, simple: 0x01, adaptive: false };
    case "commuting":
      return { scene: { mode: 0x03, subScene: lv, noiseValue: 90 }, simple: 0x01, adaptive: false };
    case "noisy":
      return { scene: { mode: 0x04, subScene: lv, noiseValue: 110 }, simple: 0x01, adaptive: false };
    case "transparency":
      return { scene: { mode: 0x0a, subScene: Math.max(1, Math.min(7, level)), noiseValue: 0 }, simple: 0x03, adaptive: false };
  }
}

export function currentNoiseUi(device: DeviceLiveState): NoiseUiMode {
  if (device.adaptive) return "adaptive";
  if (device.noiseMode === 0x00) return "off";
  if (device.noiseMode === 0x03 || device.ancScene.mode === 0x0a) return "transparency";
  if (device.ancScene.mode === 0x03) return "commuting";
  if (device.ancScene.mode === 0x04) return "noisy";
  if (device.noiseMode === 0x01) return "anc";
  return "anc";
}

function maskMac(mac: string, hide: boolean): string {
  if (!hide) return mac;
  const parts = mac.split(":");
  if (parts.length < 3) return "••:••:••:••:••:••";
  return `${parts[0]}:${parts[1]}:••:••:••:${parts[parts.length - 1]}`;
}

export const useHub = create<HubState & HubActions>((setState, get) => {
  // Route a device write and surface policy denials as a toast instead of an
  // unhandled rejection. Returns true when the write was authorized and sent.
  // Routes device writes through the per-connection scheduler (issue #10): serialized,
  // optionally coalesced, and mapped to structured results. Returns true when the write
  // was authorized and sent/confirmed; false when denied, superseded by a newer coalesced
  // value, cancelled, or errored.
  const guard = async (
    write: () => Promise<void>,
    opts?: { key?: string; coalesce?: boolean },
  ): Promise<boolean> => {
    const result = await scheduler.schedule(
      { key: opts?.key ?? `write-${++writeSeq}`, coalesce: opts?.coalesce ?? false },
      async (): Promise<CommandResult> => {
        try {
          await write();
          return { status: "sent" };
        } catch (err) {
          if (err instanceof WriteDeniedError) {
            return { status: "denied", message: err.denial.message };
          }
          throw err;
        }
      },
    );
    if (result.status === "denied") {
      setState({
        toast: { id: toastSeq++, title: "Write blocked", body: result.message ?? "" },
      });
      return false;
    }
    if (result.status === "error") {
      setState({
        toast: { id: toastSeq++, title: "Write failed", body: result.message ?? "Unknown error" },
      });
      return false;
    }
    return result.status === "sent" || result.status === "confirmed";
  };

  return {
  view: "overview",
  // Persisted config is fully validated by the schema layer (issue #11); no fallbacks
  // are needed here because loadPersistedConfig always returns a complete config.
  theme: persisted.theme,
  transportKind: "mock",
  scanning: false,
  discovered: [],
  device: createInitialState({ lastSeen: persisted.lastSeen }),
  log: [],
  toast: null,
  notify: persisted.notify,
  hideMac: persisted.hideMac,
  customEq: persisted.customEq,
  customProfiles: persisted.customProfiles,
  activeProfileId: persisted.activeProfileId,
  autoGame: persisted.autoGame,
  autoGameKeyword: persisted.autoGameKeyword,
  sleepTimerMin: persisted.sleepTimerMin,
  eqAb: null,
  systemEqOn: false,
  systemEqGains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  cliHistory: ["521C CLI — type help"],
  minimized: false,
  experimentalOptIn: false,
  pendingChime: null,
  chimeBlockedUntil: 0,

  setView: (view) => setState({ view }),
  setTheme: (theme) => {
    setState({ theme });
    persistFrom(get);
    if (typeof document !== "undefined") {
      document.documentElement.classList.toggle("light", theme === "light");
      document.documentElement.classList.toggle("dark", theme === "dark");
    }
  },
  setMinimized: (minimized) => setState({ minimized }),
  setHideMac: (hideMac) => {
    setState({ hideMac });
    persistFrom(get);
  },
  setNotify: (partial) => {
    setState({ notify: { ...get().notify, ...partial } });
    persistFrom(get);
  },
  setAutoGame: (autoGame, keyword) => {
    setState({ autoGame, autoGameKeyword: keyword ?? get().autoGameKeyword });
    persistFrom(get);
  },
  setSystemEq: (systemEqOn, gains) => {
    setState({ systemEqOn, systemEqGains: gains ?? get().systemEqGains });
  },
  snapshotEqAb: (slot) => {
    const gains = get().device.eq.bands.map((b) => b.gainDb);
    const cur = get().eqAb ?? { a: gains, b: gains, using: "a" as const };
    setState({ eqAb: { ...cur, [slot]: gains } });
  },
  toggleEqAb: () => {
    const cur = get().eqAb;
    if (!cur) return;
    const next = cur.using === "a" ? "b" : "a";
    void get().setEqGains(next === "a" ? cur.a : cur.b, `A/B ${next.toUpperCase()}`);
    setState({ eqAb: { ...cur, using: next } });
  },
  setWorn: (left, right) => {
    if (transport instanceof MockTransport) transport.setWorn(left, right);
  },
  clearLog: () => setState({ log: [] }),
  dismissToast: () => setState({ toast: null }),
  setExperimentalOptIn: (on) => {
    // Session-scoped only: never persisted, and reset whenever a transport is
    // (re)created. Propagated to the transport so the write policy sees it.
    transport.setExperimentalOptIn(on);
    setState({ experimentalOptIn: on });
  },

  scan: async () => {
    setState({ scanning: true });
    try {
      const list = await transport.scan();
      setState({ discovered: list, scanning: false });
    } catch (err) {
      setState({ scanning: false });
      setState({
        toast: {
          id: toastSeq++,
          title: "Scan failed",
          body: err instanceof Error ? err.message : "Unknown error",
        },
      });
    }
  },

  connect: async (id, kind) => {
    // A new connection gets a fresh scheduler; queued work from a previous connection
    // is cancelled rather than replayed against a different device (issue #10).
    scheduler.dispose();
    scheduler = new CommandScheduler();
    if (kind && kind !== get().transportKind) {
      transport = kind === "web-bluetooth" ? new WebBluetoothTransport() : new MockTransport();
      // A fresh transport starts a fresh session: experimental opt-in resets.
      setState({ transportKind: kind, experimentalOptIn: false });
    }
    const events = {
      onState: (device: DeviceLiveState) => {
        const prev = get().device;
        setState({ device });
        const n = get().notify;
        // Battery alerts only fire on observed telemetry, never on unknown/placeholder
        // values from a freshly connected real device (issue #2).
        if (device.telemetryKnown) {
          if (n.batteryCritical && device.battery.left.level <= 8 && prev.battery.left.level > 8) {
            setState({ toast: { id: toastSeq++, title: "Critical battery", body: "Left bud is at 8% or below." } });
          } else if (n.batteryLow && device.battery.left.level <= 20 && prev.battery.left.level > 20) {
            setState({ toast: { id: toastSeq++, title: "Low battery", body: "Left bud below 20%." } });
          }
          const uneven = Math.abs(device.battery.left.level - device.battery.right.level) >= 25;
          const wasUneven = Math.abs(prev.battery.left.level - prev.battery.right.level) >= 25;
          if (n.batteryUneven && uneven && !wasUneven) {
            setState({ toast: { id: toastSeq++, title: "Uneven battery", body: "Left and right differ by 25% or more." } });
          }
        }
      },
      onLog: (entry: PacketLogEntry) => {
        setState({ log: [...get().log, entry].slice(-400) });
      },
      onDisconnected: () => {
        scheduler.cancelQueued();
        setState({ device: { ...get().device, connected: false, connecting: false } });
        if (get().notify.disconnected) {
          setState({ toast: { id: toastSeq++, title: "Disconnected", body: get().device.name } });
        }
      },
    };
    await transport.connect(id, events);
    if (get().notify.connected) {
      setState({ toast: { id: toastSeq++, title: "Connected", body: get().device.name } });
    }
    persistFrom(get);
  },

  connectWebBluetooth: async () => {
    if (!webBluetoothAvailable()) {
      setState({
        toast: {
          id: toastSeq++,
          title: "Web Bluetooth unavailable",
          body: "Requires a Chromium browser and a secure context (HTTPS or localhost).",
        },
      });
      return;
    }
    // A real session starts fresh: new transport with unknown telemetry. scan() must run
    // on the Web Bluetooth transport so requestDevice happens inside this user gesture.
    transport = new WebBluetoothTransport();
    setState({ transportKind: "web-bluetooth", experimentalOptIn: false });
    await get().scan();
    const d = get().discovered[0];
    if (!d) {
      setState({
        toast: { id: toastSeq++, title: "No device selected", body: "Choose a QCY device to connect." },
      });
      return;
    }
    await get().connect(d.id, "web-bluetooth");
  },

  connectMock: async () => {
    transport = new MockTransport();
    setState({ transportKind: "mock", experimentalOptIn: false });
    await get().scan();
    const d = get().discovered[0];
    if (d) await get().connect(d.id, "mock");
  },

  disconnect: async () => {
    scheduler.cancelQueued("disconnect requested");
    await transport.disconnect();
  },

  setNoise: async (mode, level) => {
    const lv = level ?? (mode === "transparency" ? get().device.ancScene.subScene : get().device.ancScene.subScene);
    const mapped = noiseToScene(mode, lv);
    return guard(async () => {
      if (mapped.adaptive) {
        await transport.write(set.envAdaptation("on"));
        await transport.write(set.noiseMode(0x01));
      } else {
        await transport.write(set.envAdaptation("off"));
        await transport.write(set.noiseMode(mapped.simple));
        await transport.write(set.ancSetting(mapped.scene));
      }
    }, { key: "noise", coalesce: true });
  },

  setGameMode: async (on) => {
    return guard(() => transport.write(set.lowLatency(on ? "on" : "off")));
  },
  setSleep: async (on) => {
    await guard(() => transport.write(set.sleep(on ? "on" : "off")));
  },
  setSpatial: async (on) => {
    await guard(() => transport.write(set.spatial(on ? "on" : "off")));
  },
  setInEar: async (on) => {
    return guard(() => transport.write(set.inEar(on ? "on" : "off")));
  },
  setWear: async (partial) => {
    const wear = { ...get().device.wear, ...partial };
    await guard(() => transport.write(set.wear(wear)));
  },
  setEq: async (named) => {
    const ok = await guard(() => transport.write(set.eqV2(named.preset)));
    if (ok && transport instanceof MockTransport) transport.applyLocalEqName(named.name);
    return ok;
  },
  setEqGains: async (gains, name = "Custom") => {
    const preset = presetFromGains(gains);
    await guard(
      async () => {
        await transport.write(set.eqV2(preset));
        if (transport instanceof MockTransport) transport.applyLocalEqName(name);
      },
      { key: "eq", coalesce: true },
    );
  },
  setBinding: async (keyId, funId) => {
    const bindings: KeyBinding[] = get().device.bindings.map((b) =>
      b.keyId === keyId ? { keyId, funId } : b,
    );
    await guard(() => transport.writeDirect(CHAR.keyFunctionV2, encodeKeyFunctionDirect(bindings)));
  },
  setSoundBalance: async (value) => {
    await guard(() => transport.write(set.soundBalance(value)), { key: "balance", coalesce: true });
  },
  requestChime: (side) => {
    // Preflight only: stage the decision for interactive confirmation. No TX here.
    const d = get().device;
    const preflight = evaluateChimePreflight(side, {
      detectionEnabled: d.wear.enabled,
      wornLeft: d.wornLeft,
      wornRight: d.wornRight,
    });
    setState({ pendingChime: preflight });
  },
  cancelChime: () => setState({ pendingChime: null }),
  confirmChime: async () => {
    const pre = get().pendingChime;
    if (!pre) return;
    // Known-worn targets are blocked by default; confirmation cannot override them.
    if (pre.status === "blocked-worn") {
      setState({
        toast: { id: toastSeq++, title: "Chime blocked", body: pre.reason },
      });
      return;
    }
    if (!get().device.connected) {
      setState({
        toast: { id: toastSeq++, title: "Not connected", body: "Connect to the earbuds before using Find." },
      });
      return;
    }
    const now = Date.now();
    if (now < get().chimeBlockedUntil) {
      setState({
        toast: { id: toastSeq++, title: "Chime cooldown", body: "Wait a few seconds before chiming again." },
      });
      return;
    }
    const ok = await guard(async () => {
      await transport.write(set.lightFlash(true));
      await transport.write(set.tonePlay(chimeToneId(pre.side)));
    });
    if (!ok) return;
    setState({
      pendingChime: null,
      chimeBlockedUntil: now + CHIME_COOLDOWN_MS,
      toast: {
        id: toastSeq++,
        title: "Find earbuds",
        body: "Playing a locator tone. Keep the volume safe.",
      },
    });
  },
  applyProfile: async (id) => {
    const all = [...BUILTIN_PROFILES, ...get().customProfiles];
    const profile = all.find((p) => p.id === id);
    if (!profile) return null;
    // Run the profile's device changes as sequenced steps and record each outcome
    // (issue #10): a partial failure is reported, not silently swallowed.
    const steps: ProfileStepResult[] = [];
    const noiseOk = await get().setNoise(
      profile.noise,
      profile.noise === "transparency" ? profile.transparencyLevel : profile.ancLevel,
    );
    steps.push({ step: "noise", ok: noiseOk });
    const gameOk = await get().setGameMode(profile.gameMode);
    steps.push({ step: "gameMode", ok: gameOk });
    const inEarOk = await get().setInEar(profile.wearDetection);
    steps.push({ step: "wearDetection", ok: inEarOk });
    const eq = [...DEVICE_EQ_PRESETS, ...get().customEq].find((e) => e.id === profile.eqId);
    if (eq) {
      const eqOk = await get().setEq(eq);
      steps.push({ step: "eq", ok: eqOk });
    }
    const ok = steps.every((s) => s.ok);
    setState({ activeProfileId: id });
    persistFrom(get);
    if (get().notify.profileSwitch) {
      const failed = steps.filter((s) => !s.ok).map((s) => s.step);
      setState({
        toast: {
          id: toastSeq++,
          title: ok ? "Profile" : "Profile partially applied",
          body: ok ? `${profile.name} applied` : `${profile.name}: ${failed.join(", ")} not applied`,
        },
      });
    }
    // Reconcile against the final observed device state.
    const d = get().device;
    return {
      profileId: id,
      profileName: profile.name,
      steps,
      ok,
      observed: { noiseMode: d.noiseMode, gameMode: d.gameMode, inEar: d.inEar, eqName: d.eqName },
    };
  },
  saveCustomProfile: (profile) => {
    const rest = get().customProfiles.filter((p) => p.id !== profile.id);
    setState({ customProfiles: [...rest, profile] });
    persistFrom(get);
  },
  media: async (action) => {
    // Media control is a Linux HOST feature (MPRIS), not a QCY device command
    // (issue #13). MusicControl (0x04) has no trusted evidence and is never written
    // to the earbuds. The native runtime exposes MPRIS via `521cctl media ...`;
    // the browser preview cannot reach the session bus, so it explains instead of
    // pretending to control playback.
    setState({
      toast: {
        id: toastSeq++,
        title: "Media control is a host feature",
        body: `MPRIS media control ("${action}") runs in the native runtime — see \`521cctl media ${action}\`. The browser preview does not control playback.`,
      },
    });
  },
  exportConfig: () => {
    // Portable fields only (issue #11): local-only fields (hideMac, sleepTimerMin) and
    // runtime state (lastSeen, device identifiers, log) are excluded by the privacy
    // contract. buildExport stamps the schema version.
    const s = get();
    return buildExport({
      schema: CONFIG_SCHEMA_VERSION,
      theme: s.theme,
      notify: s.notify,
      customEq: s.customEq,
      customProfiles: s.customProfiles,
      activeProfileId: s.activeProfileId,
      autoGame: s.autoGame,
      autoGameKeyword: s.autoGameKeyword,
    });
  },
  importConfig: (json) => {
    // Validate atomically before touching any state (issue #11). An invalid file is
    // rejected with structured errors and no partial mutation; the failure is surfaced
    // as a toast as well as in the returned result.
    const result = parseImport(json);
    if (!result.ok) {
      setState({
        toast: {
          id: toastSeq++,
          title: "Import rejected",
          body: summarizeErrors(result.errors),
        },
      });
      return result;
    }
    const cfg = result.value;
    setState({
      theme: cfg.theme,
      notify: cfg.notify,
      customEq: cfg.customEq,
      customProfiles: cfg.customProfiles,
      activeProfileId: cfg.activeProfileId,
      autoGame: cfg.autoGame,
      autoGameKeyword: cfg.autoGameKeyword,
      toast: { id: toastSeq++, title: "Imported", body: "Configuration loaded and validated." },
    });
    persistFrom(get);
    return result;
  },
  exportDiagnostics: () => {
    const s = get();
    const mac = maskMac(s.device.address, s.hideMac);
    return JSON.stringify(
      {
        app: "521c",
        unofficial: true,
        profile: s.device.profile.id,
        name: s.device.name,
        address: mac,
        firmware: s.device.firmware,
        capabilities: s.device.profile.capabilities,
        log: s.log.slice(-80).map((e) => ({ ...e, hex: e.hex })),
      },
      null,
      2,
    );
  },
  runCli: async (line) => {
    const out = await runCliLine(line, get);
    setState({ cliHistory: [...get().cliHistory, `> ${line}`, out].slice(-200) });
    return out;
  },
  };
});

async function runCliLine(line: string, get: () => HubState & HubActions): Promise<string> {
  const parts = line.trim().split(/\s+/);
  const cmd = (parts[0] ?? "").toLowerCase();
  const a = get();
  switch (cmd) {
    case "":
      return "";
    case "help":
      return [
        "status  battery  anc on|off|adaptive|indoor|commuting|noisy",
        "transparency on|off  game-mode on|off  profile <id>",
        "eq set <preset>  find [left|right|both]  sleep on|off",
        "connect  disconnect  scan",
      ].join("\n");
    case "status": {
      const d = a.device;
      return [
        `${d.name} (${d.profile.subtitle})`,
        d.connected ? "connected" : "disconnected",
        `codec ${d.audio.codec} ${d.audio.sampleRate ?? "—"} Hz`,
        `rssi ${d.rssi} dBm`,
        `fw ${d.firmware.left}`,
        `noise ${currentNoiseUi(d)}  game ${d.gameMode ? "on" : "off"}`,
      ].join("\n");
    }
    case "battery": {
      const b = a.device.battery;
      return `L ${b.left.level}%${b.left.charging ? " charging" : ""}  R ${b.right.level}%${b.right.charging ? " charging" : ""}  case ${b.case.level}%`;
    }
    case "anc": {
      const raw = (parts[1] ?? "on").toLowerCase();
      const mapped: NoiseUiMode =
        raw === "off"
          ? "off"
          : raw === "adaptive"
            ? "adaptive"
            : raw === "indoor"
              ? "indoor"
              : raw === "commuting"
                ? "commuting"
                : raw === "noisy"
                  ? "noisy"
                  : raw === "transparency"
                    ? "transparency"
                    : "anc";
      await a.setNoise(mapped);
      return `anc ${raw}`;
    }
    case "transparency":
      await a.setNoise(parts[1] === "off" ? "off" : "transparency");
      return `transparency ${parts[1] ?? "on"}`;
    case "game-mode":
      await a.setGameMode(parts[1] !== "off");
      return `game-mode ${parts[1] !== "off" ? "on" : "off"}`;
    case "profile":
      if (!parts[1]) return BUILTIN_PROFILES.map((p) => p.id).join(" ");
      await a.applyProfile(parts[1]);
      return `profile ${parts[1]}`;
    case "eq":
      if (parts[1] === "set" && parts[2]) {
        const named = DEVICE_EQ_PRESETS.find((e) => e.id === parts[2] || e.name.toLowerCase() === parts[2].toLowerCase());
        if (!named) return "unknown preset";
        await a.setEq(named);
        return `eq ${named.name}`;
      }
      return DEVICE_EQ_PRESETS.map((e) => e.id).join(" ");
    case "find":
      // Audible locator actions are intentionally unavailable through the unattended
      // CLI/automation path (issue #9). They require interactive confirmation in the UI.
      return "refused: chime requires interactive confirmation in the Find view";
    case "sleep":
      await a.setSleep(parts[1] !== "off");
      return `sleep ${parts[1] !== "off" ? "on" : "off"}`;
    case "scan":
      await a.scan();
      return get().discovered.map((d) => `${d.name} ${d.address}`).join("\n") || "none";
    case "connect":
      await a.scan();
      if (get().discovered[0]) await a.connect(get().discovered[0]!.id);
      return get().device.connected ? "connected" : "failed";
    case "disconnect":
      await a.disconnect();
      return "disconnected";
    case "request":
      if (DESTRUCTIVE_CMDS.has(Number(parts[1]))) return "refused: destructive opcode";
      return "ok";
    default:
      return `unknown command: ${cmd}`;
  }
}

export { webBluetoothAvailable, maskMac };
