import { create } from "zustand";
import { CHAR, DESTRUCTIVE_CMDS, encodeKeyFunctionDirect, set } from "./protocol";
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

export type ThemeMode = "dark" | "light";

export type NotifyPrefs = {
  connected: boolean;
  disconnected: boolean;
  batteryLow: boolean;
  batteryCritical: boolean;
  batteryUneven: boolean;
  profileSwitch: boolean;
};

const STORAGE_KEY = "521c-config-v1";

type Persisted = {
  theme: ThemeMode;
  notify: NotifyPrefs;
  hideMac: boolean;
  customEq: NamedEq[];
  customProfiles: SmartProfile[];
  activeProfileId: string;
  autoGame: boolean;
  autoGameKeyword: string;
  sleepTimerMin: number;
  lastSeen: DeviceLiveState["lastSeen"];
};

function loadPersisted(): Partial<Persisted> {
  if (typeof localStorage === "undefined") return {};
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? (JSON.parse(raw) as Persisted) : {};
  } catch {
    return {};
  }
}

function savePersisted(p: Persisted) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(p));
  } catch {
    /* ignore quota */
  }
}

const persisted = loadPersisted();

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
};

export type HubActions = {
  setView: (view: HubView) => void;
  setTheme: (theme: ThemeMode) => void;
  scan: () => Promise<void>;
  connect: (id: string, kind?: TransportKind) => Promise<void>;
  disconnect: () => Promise<void>;
  setNoise: (mode: NoiseUiMode, level?: number) => Promise<void>;
  setGameMode: (on: boolean) => Promise<void>;
  setSleep: (on: boolean) => Promise<void>;
  setSpatial: (on: boolean) => Promise<void>;
  setInEar: (on: boolean) => Promise<void>;
  setWear: (partial: Partial<DeviceLiveState["wear"]>) => Promise<void>;
  setEq: (named: NamedEq) => Promise<void>;
  setEqGains: (gains: number[], name?: string) => Promise<void>;
  setBinding: (keyId: number, funId: number) => Promise<void>;
  setSoundBalance: (value: number) => Promise<void>;
  findChime: (side: "left" | "right" | "both") => Promise<void>;
  applyProfile: (id: string) => Promise<void>;
  saveCustomProfile: (profile: SmartProfile) => void;
  media: (action: "play" | "pause" | "prev" | "next") => Promise<void>;
  runCli: (line: string) => Promise<string>;
  exportConfig: () => string;
  importConfig: (json: string) => void;
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
};

let transport: QcyTransport = new MockTransport();
let toastSeq = 1;

function persistFrom(get: () => HubState) {
  const s = get();
  savePersisted({
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

export const useHub = create<HubState & HubActions>((setState, get) => ({
  view: "overview",
  theme: persisted.theme ?? "dark",
  transportKind: "mock",
  scanning: false,
  discovered: [],
  device: createInitialState({ lastSeen: persisted.lastSeen ?? null }),
  log: [],
  toast: null,
  notify: persisted.notify ?? {
    connected: true,
    disconnected: true,
    batteryLow: true,
    batteryCritical: true,
    batteryUneven: true,
    profileSwitch: true,
  },
  hideMac: persisted.hideMac ?? true,
  customEq: persisted.customEq ?? [],
  customProfiles: persisted.customProfiles ?? [],
  activeProfileId: persisted.activeProfileId ?? "music",
  autoGame: persisted.autoGame ?? false,
  autoGameKeyword: persisted.autoGameKeyword ?? "game",
  sleepTimerMin: persisted.sleepTimerMin ?? 30,
  eqAb: null,
  systemEqOn: false,
  systemEqGains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  cliHistory: ["521C CLI — type help"],
  minimized: false,

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
    if (kind && kind !== get().transportKind) {
      transport = kind === "web-bluetooth" ? new WebBluetoothTransport() : new MockTransport();
      setState({ transportKind: kind });
    }
    const events = {
      onState: (device: DeviceLiveState) => {
        const prev = get().device;
        setState({ device });
        const n = get().notify;
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
      },
      onLog: (entry: PacketLogEntry) => {
        setState({ log: [...get().log, entry].slice(-400) });
      },
      onDisconnected: () => {
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

  disconnect: async () => {
    await transport.disconnect();
  },

  setNoise: async (mode, level) => {
    const lv = level ?? (mode === "transparency" ? get().device.ancScene.subScene : get().device.ancScene.subScene);
    const mapped = noiseToScene(mode, lv);
    if (mapped.adaptive) {
      await transport.write(set.envAdaptation("on"));
      await transport.write(set.noiseMode(0x01));
    } else {
      await transport.write(set.envAdaptation("off"));
      await transport.write(set.noiseMode(mapped.simple));
      await transport.write(set.ancSetting(mapped.scene));
    }
  },

  setGameMode: async (on) => {
    await transport.write(set.lowLatency(on ? "on" : "off"));
  },
  setSleep: async (on) => {
    await transport.write(set.sleep(on ? "on" : "off"));
  },
  setSpatial: async (on) => {
    await transport.write(set.spatial(on ? "on" : "off"));
  },
  setInEar: async (on) => {
    await transport.write(set.inEar(on ? "on" : "off"));
  },
  setWear: async (partial) => {
    const wear = { ...get().device.wear, ...partial };
    await transport.write(set.wear(wear));
  },
  setEq: async (named) => {
    await transport.write(set.eqV2(named.preset));
    if (transport instanceof MockTransport) transport.applyLocalEqName(named.name);
  },
  setEqGains: async (gains, name = "Custom") => {
    const preset = presetFromGains(gains);
    await transport.write(set.eqV2(preset));
    if (transport instanceof MockTransport) transport.applyLocalEqName(name);
  },
  setBinding: async (keyId, funId) => {
    const bindings: KeyBinding[] = get().device.bindings.map((b) =>
      b.keyId === keyId ? { keyId, funId } : b,
    );
    await transport.writeDirect(CHAR.keyFunctionV2, encodeKeyFunctionDirect(bindings));
  },
  setSoundBalance: async (value) => {
    await transport.write(set.soundBalance(value));
  },
  findChime: async (side) => {
    await transport.write(set.lightFlash(true));
    await transport.write(set.tonePlay(side === "left" ? 1 : side === "right" ? 2 : 3));
    setState({
      toast: {
        id: toastSeq++,
        title: "Find earbuds",
        body: "Playing a locator tone. Do not run this at high volume while wearing the buds.",
      },
    });
  },
  applyProfile: async (id) => {
    const all = [...BUILTIN_PROFILES, ...get().customProfiles];
    const profile = all.find((p) => p.id === id);
    if (!profile) return;
    await get().setNoise(profile.noise, profile.noise === "transparency" ? profile.transparencyLevel : profile.ancLevel);
    await get().setGameMode(profile.gameMode);
    await get().setInEar(profile.wearDetection);
    const eq = [...DEVICE_EQ_PRESETS, ...get().customEq].find((e) => e.id === profile.eqId);
    if (eq) await get().setEq(eq);
    setState({ activeProfileId: id });
    persistFrom(get);
    if (get().notify.profileSwitch) {
      setState({ toast: { id: toastSeq++, title: "Profile", body: `${profile.name} applied` } });
    }
  },
  saveCustomProfile: (profile) => {
    const rest = get().customProfiles.filter((p) => p.id !== profile.id);
    setState({ customProfiles: [...rest, profile] });
    persistFrom(get);
  },
  media: async (action) => {
    const map = { play: 0x01, pause: 0x02, prev: 0x03, next: 0x04 } as const;
    await transport.write(set.music(map[action]));
  },
  exportConfig: () => {
    const s = get();
    return JSON.stringify(
      {
        theme: s.theme,
        notify: s.notify,
        customEq: s.customEq,
        customProfiles: s.customProfiles,
        activeProfileId: s.activeProfileId,
        autoGame: s.autoGame,
        autoGameKeyword: s.autoGameKeyword,
      },
      null,
      2,
    );
  },
  importConfig: (json) => {
    const data = JSON.parse(json) as Partial<Persisted>;
    setState({
      theme: data.theme ?? get().theme,
      notify: data.notify ?? get().notify,
      customEq: data.customEq ?? get().customEq,
      customProfiles: data.customProfiles ?? get().customProfiles,
      activeProfileId: data.activeProfileId ?? get().activeProfileId,
      autoGame: data.autoGame ?? get().autoGame,
      autoGameKeyword: data.autoGameKeyword ?? get().autoGameKeyword,
    });
    persistFrom(get);
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
}));

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
      await a.findChime((parts[1] as "left" | "right" | "both") ?? "both");
      return "chime sent";
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
