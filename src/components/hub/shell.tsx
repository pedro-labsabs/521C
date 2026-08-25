import { useEffect } from "react";
import {
  AudioLines,
  Bluetooth,
  Command,
  Cpu,
  Equal,
  Hand,
  LayoutDashboard,
  Moon,
  Settings2,
  SlidersHorizontal,
  Sun,
  Terminal,
  Volume2,
  X,
} from "lucide-react";
import {
  AdvancedView,
  CliView,
  ControlsView,
  DeveloperView,
  DeviceView,
  NoiseView,
  OverviewView,
  ProfilesView,
  SoundView,
} from "@/components/hub/views";
import { Button } from "@/components/ui/button";
import { currentNoiseUi, useHub, type HubView } from "@/lib/qcy/hub-store";
import { webBluetoothAvailable } from "@/lib/qcy/transport";
import { cn } from "@/lib/utils";

/** Battery percent label; "--" until real telemetry has been observed (issue #2). */
function bpct(known: boolean, level: number): string {
  return known ? `${level}%` : "--";
}

const NAV: { id: HubView; label: string; icon: typeof LayoutDashboard }[] = [
  { id: "overview", label: "Overview", icon: LayoutDashboard },
  { id: "noise", label: "Noise", icon: Volume2 },
  { id: "sound", label: "Sound", icon: AudioLines },
  { id: "controls", label: "Controls", icon: Hand },
  { id: "profiles", label: "Profiles", icon: SlidersHorizontal },
  { id: "device", label: "Device", icon: Bluetooth },
  { id: "advanced", label: "Advanced", icon: Settings2 },
  { id: "cli", label: "CLI", icon: Terminal },
  { id: "developer", label: "Developer", icon: Cpu },
];

export function HubShell() {
  const view = useHub((s) => s.view);
  const setView = useHub((s) => s.setView);
  const theme = useHub((s) => s.theme);
  const setTheme = useHub((s) => s.setTheme);
  const device = useHub((s) => s.device);
  const scan = useHub((s) => s.scan);
  const connect = useHub((s) => s.connect);
  const disconnect = useHub((s) => s.disconnect);
  const toast = useHub((s) => s.toast);
  const dismissToast = useHub((s) => s.dismissToast);
  const minimized = useHub((s) => s.minimized);
  const setMinimized = useHub((s) => s.setMinimized);
  const setGameMode = useHub((s) => s.setGameMode);
  const setNoise = useHub((s) => s.setNoise);
  const transportKind = useHub((s) => s.transportKind);
  const connectWebBluetooth = useHub((s) => s.connectWebBluetooth);

  useEffect(() => {
    document.documentElement.classList.toggle("light", theme === "light");
    document.documentElement.classList.toggle("dark", theme === "dark");
  }, [theme]);

  useEffect(() => {
    void (async () => {
      await scan();
      const first = useHub.getState().discovered[0];
      if (first && !useHub.getState().device.connected) {
        await connect(first.id, first.kind);
      }
    })();
  }, [scan, connect]);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(dismissToast, 4200);
    return () => clearTimeout(t);
  }, [toast, dismissToast]);

  if (minimized) {
    return (
      <div className="flex min-h-dvh items-end justify-center bg-bg p-4 sm:items-start sm:justify-end sm:p-6">
        <button
          type="button"
          onClick={() => setMinimized(false)}
          className="flex w-full max-w-sm items-center gap-3 rounded-xl border border-border bg-bg-elevated px-4 py-3 text-left shadow-[var(--shadow-panel)]"
        >
          <span className="size-2.5 rounded-full bg-accent" />
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-semibold">521C</div>
            <div className="truncate text-xs text-fg-muted">
              L {bpct(device.telemetryKnown, device.battery.left.level)} · R {bpct(device.telemetryKnown, device.battery.right.level)} · {currentNoiseUi(device)}
            </div>
          </div>
        </button>
      </div>
    );
  }

  return (
    <div className="min-h-dvh bg-bg text-fg">
      <div className="mx-auto flex min-h-dvh max-w-6xl flex-col">
        <header className="flex items-center gap-3 border-b border-border px-3 py-2.5 sm:px-4">
          <div className="flex size-8 items-center justify-center rounded-md bg-accent text-accent-fg">
            <Equal className="size-4" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <h1 className="text-sm font-semibold tracking-tight">521C</h1>
              <span className="hidden text-xs text-fg-subtle sm:inline">Unofficial</span>
            </div>
            <p className="truncate text-xs text-fg-muted">
              {device.profile.subtitle} · {device.connected ? "connected" : device.connecting ? "connecting" : "idle"}
              <span
                className={cn(
                  "ml-2 rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
                  transportKind === "web-bluetooth" ? "bg-accent/15 text-accent" : "bg-bg-hover text-fg-subtle",
                )}
              >
                {transportKind === "web-bluetooth" ? "Web Bluetooth" : "Mock preview"}
              </span>
            </p>
          </div>
          <div className="hidden items-center gap-2 tabular text-xs text-fg-muted md:flex">
            <span>L {bpct(device.telemetryKnown, device.battery.left.level)}</span>
            <span>R {bpct(device.telemetryKnown, device.battery.right.level)}</span>
            <span>Case {bpct(device.telemetryKnown, device.battery.case.level)}</span>
          </div>
          <Button size="sm" variant="quiet" onClick={() => setTheme(theme === "dark" ? "light" : "dark")} aria-label="Toggle theme">
            {theme === "dark" ? <Sun className="size-4" /> : <Moon className="size-4" />}
          </Button>
          <Button size="sm" variant="quiet" onClick={() => setMinimized(true)} aria-label="Minimize to tray">
            Tray
          </Button>
        </header>

        <div className="flex min-h-0 flex-1 flex-col md:flex-row">
          <nav className="flex flex-nowrap gap-1 overflow-x-auto border-b border-border p-2 md:w-52 md:flex-col md:overflow-visible md:border-b-0 md:border-r">
            {NAV.map((item) => {
              const Icon = item.icon;
              const active = view === item.id;
              return (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => setView(item.id)}
                  className={cn(
                    "flex min-h-11 shrink-0 items-center gap-2 rounded-md px-3 text-sm",
                    active ? "bg-bg-subtle font-medium text-fg" : "text-fg-muted hover:bg-bg-hover hover:text-fg",
                  )}
                >
                  <Icon className="size-4" />
                  {item.label}
                </button>
              );
            })}
          </nav>

          <main className="hub-scroll min-h-0 flex-1 overflow-auto p-3 sm:p-5">
            {view === "overview" && <OverviewView />}
            {view === "noise" && <NoiseView />}
            {view === "sound" && <SoundView />}
            {view === "controls" && <ControlsView />}
            {view === "profiles" && <ProfilesView />}
            {view === "device" && <DeviceView />}
            {view === "advanced" && <AdvancedView />}
            {view === "cli" && <CliView />}
            {view === "developer" && <DeveloperView />}
          </main>
        </div>

        <footer className="flex flex-wrap items-center gap-x-4 gap-y-1 border-t border-border px-3 py-2 text-xs text-fg-muted sm:px-4">
          <span className="tabular">{device.audio.codec}</span>
          <span className="tabular">{device.rssi} dBm</span>
          <span>fw {device.firmware.left}</span>
          <span className="hidden sm:inline">mock</span>
          <span className="ml-auto flex gap-2">
            <button type="button" className="hover:text-fg" onClick={() => void setNoise("transparency")}>
              Transparency
            </button>
            <button type="button" className="hover:text-fg" onClick={() => void setGameMode(!device.gameMode)}>
              Game
            </button>
            {webBluetoothAvailable() && (
              <button type="button" className="hover:text-fg" onClick={() => void connectWebBluetooth()}>
                Connect real device
              </button>
            )}
            {device.connected ? (
              <button type="button" className="hover:text-fg" onClick={() => void disconnect()}>
                Disconnect
              </button>
            ) : (
              <button
                type="button"
                className="hover:text-fg"
                onClick={() => {
                  void scan().then(() => {
                    const d = useHub.getState().discovered[0];
                    if (d) void connect(d.id, d.kind);
                  });
                }}
              >
                Reconnect
              </button>
            )}
          </span>
        </footer>
      </div>

      {toast && (
        <div className="fixed bottom-16 right-4 z-50 max-w-sm rounded-lg border border-border bg-bg-elevated p-3 shadow-[var(--shadow-panel)]">
          <div className="flex items-start gap-2">
            <Command className="mt-0.5 size-4 text-accent" />
            <div className="min-w-0 flex-1">
              <div className="text-sm font-medium">{toast.title}</div>
              <div className="text-xs text-fg-muted">{toast.body}</div>
            </div>
            <button type="button" onClick={dismissToast} className="text-fg-subtle hover:text-fg" aria-label="Dismiss">
              <X className="size-4" />
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
