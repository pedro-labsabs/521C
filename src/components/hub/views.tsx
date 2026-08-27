import { useMemo, useState, type ReactNode } from "react";
import {
  Activity,
  AlertTriangle,
  Bluetooth,
  Copy,
  Download,
  Gamepad2,
  Pause,
  Play,
  SkipBack,
  SkipForward,
  Upload,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { EarbudsStage } from "@/components/hub/earbuds";
import { CapabilityChip } from "@/components/hub/capability-chip";
import { Panel, Segmented, Toggle } from "@/components/hub/panel";
import { DEVICE_EQ_PRESETS, EQ_FREQS, eqGains } from "@/lib/qcy/eq-presets";
import { HT08_CAPABILITIES, canInteract } from "@/lib/qcy/device/capabilities";
import { CHAR, FUN_LABELS, KEY_LABELS, SERVICE, cmdName } from "@/lib/qcy/protocol";
import { BUILTIN_PROFILES, type NoiseUiMode } from "@/lib/qcy/smart-profiles";
import { currentNoiseUi, useHub } from "@/lib/qcy/hub-store";
import { cn } from "@/lib/utils";

function rssiLabel(rssi: number): string {
  if (rssi >= -45) return "Very near";
  if (rssi >= -60) return "Near";
  if (rssi >= -75) return "Medium";
  return "Far";
}

export function OverviewView() {
  const device = useHub((s) => s.device);
  const setNoise = useHub((s) => s.setNoise);
  const setGameMode = useHub((s) => s.setGameMode);
  const media = useHub((s) => s.media);
  const transportKind = useHub((s) => s.transportKind);
  const mode = currentNoiseUi(device);

  return (
    <div className="grid gap-4 lg:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
      <EarbudsStage
        left={device.battery.left}
        right={device.battery.right}
        caseCell={device.battery.case}
        wornLeft={device.wornLeft}
        wornRight={device.wornRight}
        connected={device.connected}
        known={device.telemetryKnown}
      />
      <div className="grid gap-4">
        <Panel title="Now">
          <dl className="grid grid-cols-2 gap-x-4 gap-y-3 text-sm">
            <Stat k="Model" v={device.profile.subtitle} />
            <Stat k="Link" v={device.connected ? "Connected" : "Disconnected"} />
            <Stat k="Codec" v={device.audio.codec} />
            <Stat k="Profile" v={device.audio.profile} />
            <Stat k="Sample rate" v={device.audio.sampleRate ? `${device.audio.sampleRate / 1000} kHz` : "—"} />
            <Stat k="Bitrate" v="Not exposed" />
            <Stat k="RSSI" v={`${device.rssi} dBm · ${rssiLabel(device.rssi)}`} />
            <Stat k="Firmware" v={device.firmware.left} />
          </dl>
        </Panel>
        <Panel title="Quick noise">
          <Segmented
            value={mode === "indoor" || mode === "commuting" || mode === "noisy" ? "anc" : mode}
            onChange={(v) => void setNoise(v)}
            options={[
              { id: "off" as NoiseUiMode, label: "Off" },
              { id: "anc" as NoiseUiMode, label: "ANC" },
              { id: "adaptive" as NoiseUiMode, label: "Adaptive" },
              { id: "transparency" as NoiseUiMode, label: "Transparency" },
            ]}
          />
          <div className="mt-4 flex items-center justify-between">
            <div>
              <div className="text-sm font-medium">Game mode</div>
              <div className="text-xs text-fg-muted">Low latency opcode 0x09</div>
            </div>
            <Toggle checked={device.gameMode} onChange={(v) => void setGameMode(v)} label="Game mode" />
          </div>
        </Panel>
        <Panel title="Media · MPRIS (host)">
          {transportKind === "mock" ? (
            <>
              <div className="text-sm font-medium">{device.media.title}</div>
              <div className="text-xs text-fg-muted">
                {device.media.artist} · {device.media.player}
              </div>
              <div className="mt-1 text-[11px] text-fg-subtle">
                Sample data. MPRIS is a Linux host feature — the native runtime controls
                real players (521cctl media …).
              </div>
            </>
          ) : (
            <div className="text-xs text-fg-muted">
              MPRIS media state is a Linux host feature of the native runtime (521cctl
              media …). The browser path cannot reach your media player, so no state is
              shown here.
            </div>
          )}
          <div className="mt-3 flex gap-2">
            <Button
              size="icon"
              className="size-9"
              onClick={() => void media("prev")}
              aria-label="Previous (host feature, native runtime only)"
              title="MPRIS control runs in the native runtime (521cctl media prev)"
            >
              <SkipBack className="size-4" />
            </Button>
            <Button
              size="icon"
              className="size-9"
              variant="primary"
              onClick={() => void media(device.media.playing ? "pause" : "play")}
              aria-label={device.media.playing ? "Pause (host feature, native runtime only)" : "Play (host feature, native runtime only)"}
              title="MPRIS control runs in the native runtime (521cctl media play|pause)"
            >
              {device.media.playing ? <Pause className="size-4" /> : <Play className="size-4" />}
            </Button>
            <Button
              size="icon"
              className="size-9"
              onClick={() => void media("next")}
              aria-label="Next (host feature, native runtime only)"
              title="MPRIS control runs in the native runtime (521cctl media next)"
            >
              <SkipForward className="size-4" />
            </Button>
          </div>
        </Panel>
      </div>
    </div>
  );
}

function Stat({ k, v }: { k: string; v: string }) {
  return (
    <div>
      <dt className="text-xs text-fg-subtle">{k}</dt>
      <dd className="font-medium">{v}</dd>
    </div>
  );
}

export function NoiseView() {
  const device = useHub((s) => s.device);
  const setNoise = useHub((s) => s.setNoise);
  const mode = currentNoiseUi(device);
  const caps = device.profile.capabilities;

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Panel
        title="Noise control"
        description="HT08 ANC scenes via opcode 0x17 [mode, subScene, noiseValue], validated on live hardware."
      >
        <div className="grid gap-2">
          {(
            [
              ["off", "Off", caps.ancOff],
              ["anc", "ANC", caps.ancOn],
              ["adaptive", "Adaptive ANC", caps.ancAdaptive],
              ["indoor", "Indoor / silent", caps.ancIndoor],
              ["commuting", "Commuting / working", caps.ancCommuting],
              ["noisy", "Noisy environment", caps.ancNoisy],
              ["wind", "Wind reduction", caps.ancWind],
              ["transparency", "Transparency", caps.transparency],
            ] as const
          ).map(([id, label, flag]) => (
            <button
              key={id}
              type="button"
              disabled={!canInteract(flag)}
              onClick={() => void setNoise(id)}
              className={cn(
                "flex items-center justify-between rounded-lg border px-3 py-2.5 text-left text-sm",
                mode === id ? "border-accent bg-accent/10" : "border-border bg-bg hover:bg-bg-hover",
              )}
            >
              <span className="font-medium">{label}</span>
              <CapabilityChip cap={flag} />
            </button>
          ))}
        </div>
      </Panel>
      <Panel title="Scene state" description="Reported by the device through 0x17 AncSetting notifications">
        <div className="grid gap-2 text-sm">
          <div className="flex items-center justify-between rounded-lg border border-border px-3 py-2.5">
            <span className="text-fg-muted">Mode byte</span>
            <span className="font-mono">0x{device.ancScene.mode.toString(16).padStart(2, "0")}</span>
          </div>
          <div className="flex items-center justify-between rounded-lg border border-border px-3 py-2.5">
            <span className="text-fg-muted">Scene (subScene)</span>
            <span className="font-mono">{device.ancScene.subScene}</span>
          </div>
          <div className="flex items-center justify-between rounded-lg border border-border px-3 py-2.5">
            <span className="text-fg-muted">Noise value</span>
            <span className="font-mono">{device.ancScene.noiseValue}</span>
          </div>
        </div>
        <div className="mt-4 rounded-lg border border-dashed border-border px-3 py-3 text-xs text-fg-muted">
          Adjustable ANC/transparency levels are not validated on the HT08: the firmware uses one
          fixed payload per scene (wind/adaptive/transparency normalize the noise value to 0).
        </div>
      </Panel>
    </div>
  );
}

export function SoundView() {
  const device = useHub((s) => s.device);
  const setEq = useHub((s) => s.setEq);
  const setEqGains = useHub((s) => s.setEqGains);
  const snapshotEqAb = useHub((s) => s.snapshotEqAb);
  const toggleEqAb = useHub((s) => s.toggleEqAb);
  const systemEqOn = useHub((s) => s.systemEqOn);
  const systemEqGains = useHub((s) => s.systemEqGains);
  const setSystemEq = useHub((s) => s.setSystemEq);
  const setSoundBalance = useHub((s) => s.setSoundBalance);
  const gains = eqGains(device.eq);

  return (
    <div className="grid gap-4">
      <Panel
        title="Device EQ"
        description="Written to the earbuds through opcode 0x22. These are community band tables, not dumped official factory dumps."
        action={
          <div className="flex gap-2">
            <Button size="sm" onClick={() => snapshotEqAb("a")}>
              Store A
            </Button>
            <Button size="sm" onClick={() => snapshotEqAb("b")}>
              Store B
            </Button>
            <Button size="sm" variant="primary" onClick={() => toggleEqAb()}>
              A/B
            </Button>
          </div>
        }
      >
        <div className="mb-4 flex flex-wrap gap-2">
          {DEVICE_EQ_PRESETS.map((p) => (
            <Button
              key={p.id}
              size="sm"
              variant={device.eqName === p.name ? "primary" : "secondary"}
              onClick={() => void setEq(p)}
            >
              {p.name}
            </Button>
          ))}
        </div>
        <div className="grid grid-cols-5 gap-3 sm:grid-cols-10">
          {gains.map((g, i) => {
            const freq = EQ_FREQS[i]!;
            const label = freq >= 1000 ? `${freq / 1000}k` : String(freq);
            const h = Math.max(8, ((g + 12) / 24) * 100);
            return (
              <label key={freq} className="flex min-w-0 flex-col items-center gap-2">
                <div className="flex h-32 w-full items-end justify-center rounded-md bg-bg">
                  <div className="w-2.5 rounded-sm bg-accent" style={{ height: `${h}%` }} />
                </div>
                <input
                  type="range"
                  min={-12}
                  max={12}
                  step={0.5}
                  value={g}
                  onChange={(e) => {
                    const next = [...gains];
                    next[i] = Number(e.target.value);
                    void setEqGains(next);
                  }}
                  className="w-full accent-[var(--color-accent)]"
                  aria-label={`${freq} Hz`}
                />
                <span className="text-xs text-fg-subtle">{label}</span>
              </label>
            );
          })}
        </div>
      </Panel>
      <div className="grid gap-4 lg:grid-cols-2">
        <Panel
          title="System EQ"
          description="Host-side curve only. Never presented as written to the hardware."
          action={<Toggle checked={systemEqOn} onChange={(v) => setSystemEq(v)} label="System EQ" />}
        >
          <div className={cn("grid grid-cols-5 gap-2 sm:grid-cols-10", !systemEqOn && "opacity-40")}>
            {systemEqGains.map((g, i) => (
              <input
                key={EQ_FREQS[i]}
                type="range"
                min={-12}
                max={12}
                value={g}
                disabled={!systemEqOn}
                onChange={(e) => {
                  const next = [...systemEqGains];
                  next[i] = Number(e.target.value);
                  setSystemEq(true, next);
                }}
                className="w-full accent-[var(--color-accent)]"
                aria-label={`System ${EQ_FREQS[i]} Hz`}
              />
            ))}
          </div>
        </Panel>
        <Panel title="Balance" description="Opcode 0x16 · 0 left, 50 center, 100 right">
          <input
            type="range"
            min={0}
            max={100}
            value={device.soundBalance}
            onChange={(e) => void setSoundBalance(Number(e.target.value))}
            className="w-full accent-[var(--color-accent)]"
          />
          <div className="mt-2 flex justify-between text-xs text-fg-muted">
            <span>Left</span>
            <span className="tabular">{device.soundBalance}</span>
            <span>Right</span>
          </div>
        </Panel>
      </div>
    </div>
  );
}

export function ControlsView() {
  const device = useHub((s) => s.device);
  const setBinding = useHub((s) => s.setBinding);
  const setInEar = useHub((s) => s.setInEar);
  const setWear = useHub((s) => s.setWear);
  const setSleep = useHub((s) => s.setSleep);
  const setWorn = useHub((s) => s.setWorn);
  const musicKeys = device.bindings.filter((b) => b.keyId <= 0x0a);
  const funOptions = Object.entries(FUN_LABELS);

  return (
    <div className="grid gap-4">
      <Panel title="Touch mapping" description="Read before write. Direct characteristic 0000000D, no 0xFF frame.">
        <div className="grid gap-2">
          {musicKeys.map((b) => (
            <div key={b.keyId} className="grid grid-cols-[1fr_1fr] items-center gap-3 rounded-lg bg-bg px-3 py-2">
              <div className="text-sm">{KEY_LABELS[b.keyId] ?? `Key 0x${b.keyId.toString(16)}`}</div>
              <select
                className="h-9 rounded-md border border-border bg-bg-subtle px-2 text-sm"
                value={b.funId}
                onChange={(e) => void setBinding(b.keyId, Number(e.target.value))}
              >
                {funOptions.map(([id, label]) => (
                  <option key={id} value={id}>
                    {label}
                  </option>
                ))}
              </select>
            </div>
          ))}
        </div>
      </Panel>
      <div className="grid gap-4 lg:grid-cols-2">
        <Panel title="Wear detection">
          <Row label="Enabled" hint="Opcode 0x06 / 0x2C">
            <Toggle checked={device.wear.enabled} onChange={(v) => void setWear({ enabled: v })} />
          </Row>
          <Row label="In-ear sensor">
            <Toggle checked={device.inEar} onChange={(v) => void setInEar(v)} />
          </Row>
          <Row label="Simulate left worn">
            <Toggle checked={device.wornLeft} onChange={(v) => setWorn(v, device.wornRight)} />
          </Row>
          <Row label="Simulate right worn">
            <Toggle checked={device.wornRight} onChange={(v) => setWorn(device.wornLeft, v)} />
          </Row>
        </Panel>
        <Panel title="Sleep mode" description="Locks gestures, keeps audio. Opcode 0x10.">
          <Row label="Sleeping mode">
            <Toggle checked={device.sleepMode} onChange={(v) => void setSleep(v)} />
          </Row>
        </Panel>
      </div>
    </div>
  );
}

function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3 border-b border-border py-3 last:border-0">
      <div>
        <div className="text-sm font-medium">{label}</div>
        {hint && <div className="text-xs text-fg-muted">{hint}</div>}
      </div>
      {children}
    </div>
  );
}

export function ProfilesView() {
  const applyProfile = useHub((s) => s.applyProfile);
  const active = useHub((s) => s.activeProfileId);
  const autoGame = useHub((s) => s.autoGame);
  const autoGameKeyword = useHub((s) => s.autoGameKeyword);
  const setAutoGame = useHub((s) => s.setAutoGame);
  const custom = useHub((s) => s.customProfiles);

  return (
    <div className="grid gap-4">
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {[...BUILTIN_PROFILES, ...custom].map((p) => (
          <button
            key={p.id}
            type="button"
            onClick={() => void applyProfile(p.id)}
            className={cn(
              "rounded-xl border p-4 text-left",
              active === p.id ? "border-accent bg-accent/10" : "border-border bg-bg-elevated hover:bg-bg-hover",
            )}
          >
            <div className="text-sm font-semibold">{p.name}</div>
            <p className="mt-1 text-xs text-fg-muted">{p.description}</p>
            <div className="mt-3 text-xs text-fg-subtle">
              {p.noise} · {p.gameMode ? "game on" : "game off"} · EQ {p.eqId}
            </div>
          </button>
        ))}
      </div>
      <Panel title="Auto Game Mode" description="Host-side. No aggressive polling — matches a user keyword against the current player name.">
        <Row label="Enable">
          <Toggle checked={autoGame} onChange={(v) => setAutoGame(v)} />
        </Row>
        <label className="mt-2 block text-xs text-fg-muted">
          Trigger keyword
          <input
            className="mt-1 h-10 w-full rounded-md border border-border bg-bg px-3 text-sm text-fg"
            value={autoGameKeyword}
            onChange={(e) => setAutoGame(autoGame, e.target.value)}
          />
        </label>
      </Panel>
    </div>
  );
}

function FindEarbudsPanel() {
  const device = useHub((s) => s.device);
  const pre = useHub((s) => s.pendingChime);
  const requestChime = useHub((s) => s.requestChime);
  const cancelChime = useHub((s) => s.cancelChime);
  const confirmChime = useHub((s) => s.confirmChime);
  const [strongAck, setStrongAck] = useState(false);

  const start = (side: "left" | "right" | "both") => {
    setStrongAck(false);
    requestChime(side);
  };

  const canPlay =
    !!pre && pre.status !== "blocked-worn" && (pre.status !== "confirm-strong" || strongAck);

  const sideLabel = pre
    ? pre.side === "both"
      ? "both buds"
      : pre.side === "left"
        ? "the left bud"
        : "the right bud"
    : "";

  return (
    <Panel
      title="Find earbuds"
      description="Chime uses 0x05 / 0x3D. Proximity is smoothed host RSSI — HT08 has no GPS."
    >
      <div className="mb-4">
        <div className="text-xs text-fg-muted">Proximity</div>
        <div className="mt-1 text-lg font-semibold">{rssiLabel(device.rssi)}</div>
        <div className="tabular text-xs text-fg-subtle">{device.rssi} dBm</div>
      </div>

      {!pre && (
        <>
          <div className="flex flex-wrap gap-2">
            <Button disabled={!device.connected} onClick={() => start("left")}>
              Chime left
            </Button>
            <Button disabled={!device.connected} onClick={() => start("right")}>
              Chime right
            </Button>
            <Button variant="primary" disabled={!device.connected} onClick={() => start("both")}>
              Chime both
            </Button>
          </div>
          <p className="mt-3 text-xs text-fg-muted">
            Chiming asks for confirmation first and is blocked while a target bud is worn.
          </p>
        </>
      )}

      {pre && (
        <div className="rounded-lg border border-border bg-bg px-3 py-3">
          <div className="text-sm font-medium">Chime {sideLabel}</div>
          <p className={cn("mt-1 text-xs", pre.status === "blocked-worn" ? "text-danger" : "text-warn")}>
            {pre.reason}
          </p>
          <p className="mt-1 text-xs text-fg-muted">
            Hearing safety: do not play the locator tone at high volume while a bud is in your ear.
          </p>

          {pre.status === "confirm-strong" && (
            <label className="mt-3 flex items-start gap-2 text-xs text-fg">
              <input
                type="checkbox"
                checked={strongAck}
                onChange={(e) => setStrongAck(e.target.checked)}
                className="mt-0.5"
              />
              <span>I have checked and the target bud is not in my ear.</span>
            </label>
          )}

          <div className="mt-3 flex flex-wrap gap-2">
            {pre.status !== "blocked-worn" && (
              <Button
                variant="primary"
                disabled={!canPlay}
                onClick={() => {
                  setStrongAck(false);
                  void confirmChime();
                }}
              >
                Play chime
              </Button>
            )}
            <Button
              onClick={() => {
                setStrongAck(false);
                cancelChime();
              }}
            >
              Cancel
            </Button>
          </div>
        </div>
      )}

      {device.lastSeen && (
        <p className="mt-3 text-xs text-fg-muted">
          Last seen {new Date(device.lastSeen.at).toLocaleString()} on {device.lastSeen.host}
        </p>
      )}
    </Panel>
  );
}

export function DeviceView() {
  const device = useHub((s) => s.device);
  const setSpatial = useHub((s) => s.setSpatial);
  const hideMac = useHub((s) => s.hideMac);
  const address = hideMac
    ? device.address.replace(/(:[0-9A-F]{2}){3}:/i, ":••:••:••:")
    : device.address;

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Panel title="Identity">
        <dl className="grid gap-3 text-sm">
          <Stat k="Name" v={device.name} />
          <Stat k="Model" v={`${device.profile.title} · ${device.profile.subtitle}`} />
          <Stat k="Address" v={address} />
          <Stat k="Bluetooth" v={device.profile.bluetooth} />
          <Stat k="Drivers" v={device.profile.drivers} />
          <Stat k="Microphones" v={device.profile.mics} />
          <Stat k="Firmware L/R" v={`${device.firmware.left}${device.firmware.right ? ` / ${device.firmware.right}` : ""}`} />
        </dl>
        <div className="mt-4 rounded-lg border border-border bg-bg px-3 py-3 text-xs text-fg-muted">
          Firmware update: not yet safely supported. 521C will not send OTA payloads.
        </div>
      </Panel>
      <FindEarbudsPanel />
      <Panel title="Spatial audio" description="Opcode 0x2D exists. HT08 firmware exposure is unverified.">
        <Row label="Spatial (experimental)">
          <Toggle checked={device.spatial} onChange={(v) => void setSpatial(v)} />
        </Row>
      </Panel>
      <Panel title="Multipoint">
        <p className="text-sm text-fg-muted">
          Dual-device audio is a Bluetooth stack property. No public QCY command lists hosts or toggles multipoint. Status: unknown / needs research.
        </p>
      </Panel>
    </div>
  );
}

export function AdvancedView() {
  const notify = useHub((s) => s.notify);
  const setNotify = useHub((s) => s.setNotify);
  const hideMac = useHub((s) => s.hideMac);
  const setHideMac = useHub((s) => s.setHideMac);
  const exportConfig = useHub((s) => s.exportConfig);
  const importConfig = useHub((s) => s.importConfig);
  const experimentalOptIn = useHub((s) => s.experimentalOptIn);
  const setExperimentalOptIn = useHub((s) => s.setExperimentalOptIn);
  const caps = HT08_CAPABILITIES;

  return (
    <div className="grid gap-4">
      <Panel title="Notifications">
        {(
          [
            ["connected", "Connected"],
            ["disconnected", "Disconnected"],
            ["batteryLow", "Battery low"],
            ["batteryCritical", "Battery critical"],
            ["batteryUneven", "Uneven battery"],
            ["profileSwitch", "Profile switch"],
          ] as const
        ).map(([k, label]) => (
          <Row key={k} label={label}>
            <Toggle checked={notify[k]} onChange={(v) => setNotify({ [k]: v })} />
          </Row>
        ))}
      </Panel>
      <Panel title="Privacy & backup">
        <Row label="Hide MAC in exports">
          <Toggle checked={hideMac} onChange={setHideMac} />
        </Row>
        <div className="mt-3 flex flex-wrap gap-2">
          <Button
            onClick={() => {
              const blob = new Blob([exportConfig()], { type: "application/json" });
              const url = URL.createObjectURL(blob);
              const a = document.createElement("a");
              a.href = url;
              a.download = "521c-config.json";
              a.click();
              URL.revokeObjectURL(url);
            }}
          >
            <Download className="size-4" /> Export
          </Button>
          <label className="inline-flex">
            <input
              type="file"
              accept="application/json"
              className="hidden"
              onChange={(e) => {
                const file = e.target.files?.[0];
                if (!file) return;
                void file.text().then(importConfig);
              }}
            />
            <span className="inline-flex h-10 items-center gap-2 rounded-md border border-border bg-bg-subtle px-4 text-sm font-medium hover:bg-bg-hover">
              <Upload className="size-4" /> Import
            </span>
          </label>
        </div>
      </Panel>
      <Panel title="Experimental features">
        <Row label="Allow experimental device writes this session">
          <Toggle checked={experimentalOptIn} onChange={setExperimentalOptIn} />
        </Row>
        <p className="mt-2 text-xs text-fg-muted">
          Adaptive ANC, spatial audio and LDAC are marked experimental for HT08. Enabling
          them requires this explicit opt-in. It applies to the current session only — it is
          never saved and resets on restart or when the transport changes.
        </p>
      </Panel>
      <Panel title="HT08 capability matrix">
        <div className="grid gap-2">
          {Object.entries(caps).map(([key, flag]) => (
            <div key={key} className="flex items-start justify-between gap-3 rounded-md bg-bg px-3 py-2">
              <div>
                <div className="text-sm font-medium">{key}</div>
                <div className="text-xs text-fg-muted">{flag.note ?? "—"}</div>
              </div>
              <CapabilityChip cap={flag} />
            </div>
          ))}
        </div>
      </Panel>
    </div>
  );
}

export function CliView() {
  const history = useHub((s) => s.cliHistory);
  const runCli = useHub((s) => s.runCli);
  const [line, setLine] = useState("");

  return (
    <Panel title="521cctl" description="Same core as the GUI. Destructive opcodes are refused.">
      <div className="h-72 overflow-auto rounded-lg bg-bg p-3 font-mono text-xs leading-relaxed text-fg hub-scroll">
        {history.map((h, i) => (
          <div key={`${i}-${h.slice(0, 12)}`} className={h.startsWith(">") ? "text-accent" : "text-fg-muted"}>
            {h}
          </div>
        ))}
      </div>
      <form
        className="mt-3 flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          const v = line;
          setLine("");
          void runCli(v);
        }}
      >
        <input
          value={line}
          onChange={(e) => setLine(e.target.value)}
          placeholder="status"
          className="h-10 flex-1 rounded-md border border-border bg-bg px-3 font-mono text-sm"
        />
        <Button variant="primary" type="submit">
          Run
        </Button>
      </form>
    </Panel>
  );
}

export function DeveloperView() {
  const log = useHub((s) => s.log);
  const clearLog = useHub((s) => s.clearLog);
  const exportDiagnostics = useHub((s) => s.exportDiagnostics);
  const device = useHub((s) => s.device);
  const [copied, setCopied] = useState(false);

  const uuids = useMemo(
    () => [
      ["Main service", SERVICE.main],
      ["Command write", CHAR.commandWrite],
      ["Notify", CHAR.settingsNotify],
      ["Battery", CHAR.battery],
      ["Version", CHAR.version],
      ["EQ direct", CHAR.eqDirect],
      ["Keys V2", CHAR.keyFunctionV2],
    ],
    [],
  );

  return (
    <div className="grid gap-4">
      <Panel
        title="Packet log"
        description="Frames are validated (SOF, length, bounds) before use. Untrusted BLE input never crashes the UI."
        action={
          <div className="flex gap-2">
            <Button size="sm" onClick={clearLog}>
              Clear
            </Button>
            <Button
              size="sm"
              onClick={async () => {
                await navigator.clipboard.writeText(exportDiagnostics());
                setCopied(true);
                setTimeout(() => setCopied(false), 1200);
              }}
            >
              <Copy className="size-3.5" /> {copied ? "Copied" : "Export"}
            </Button>
          </div>
        }
      >
        <div className="h-64 overflow-auto font-mono text-xs hub-scroll">
          {log.length === 0 && <div className="text-fg-subtle">No packets yet. Connect to generate traffic.</div>}
          {log.map((e) => (
            <div key={e.id} className="grid grid-cols-[48px_1fr] gap-2 border-b border-border/60 py-1">
              <span className={e.dir === "tx" ? "text-accent" : "text-warn"}>{e.dir}</span>
              <span>
                {cmdName(e.cmd)} · {e.hex}
              </span>
            </div>
          ))}
        </div>
      </Panel>
      <Panel title="GATT map">
        <div className="grid gap-1 font-mono text-xs">
          {uuids.map(([n, u]) => (
            <div key={u} className="flex flex-wrap justify-between gap-2 rounded-md bg-bg px-3 py-2">
              <span className="text-fg-muted">{n}</span>
              <span>{u}</span>
            </div>
          ))}
        </div>
      </Panel>
      <Panel title="Truth sources">
        <ul className="space-y-2 text-sm text-fg-muted">
          <li>
            <Activity className="mr-2 inline size-4 text-accent" />
            Hardware: what HT08 actually supports.
          </li>
          <li>
            <Bluetooth className="mr-2 inline size-4 text-accent" />
            Protocol: what we can prove with public opcodes.
          </li>
          <li>
            <Gamepad2 className="mr-2 inline size-4 text-accent" />
            App: what this build implements and tests.
          </li>
          <li>
            <AlertTriangle className="mr-2 inline size-4 text-warn" />
            A button in the official app is not a Linux opcode.
          </li>
        </ul>
        <p className="mt-3 text-xs text-fg-subtle">
          Connected as {device.profile.id}. Vendor IDs are learned from manufacturer data (CompanyID 0x521c) and are not invented.
        </p>
      </Panel>
    </div>
  );
}
