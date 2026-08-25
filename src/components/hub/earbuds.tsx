import { cn } from "@/lib/utils";
import type { BatteryCell } from "@/lib/qcy/protocol/types";

function Ring({
  value,
  charging,
  label,
  known,
}: {
  value: number;
  charging: boolean;
  label: string;
  known: boolean;
}) {
  const r = 28;
  const c = 2 * Math.PI * r;
  const offset = known ? c - (Math.max(0, Math.min(100, value)) / 100) * c : c;
  const tone = !known
    ? "stroke-border"
    : value <= 15
      ? "stroke-danger"
      : value <= 30
        ? "stroke-warn"
        : "stroke-accent";
  return (
    <div className="flex flex-col items-center gap-2">
      <div className="relative size-20">
        <svg viewBox="0 0 72 72" className="size-20 -rotate-90">
          <circle cx="36" cy="36" r={r} className="fill-none stroke-border" strokeWidth="5" />
          <circle
            cx="36"
            cy="36"
            r={r}
            className={cn("fill-none", tone)}
            strokeWidth="5"
            strokeLinecap="round"
            strokeDasharray={c}
            strokeDashoffset={offset}
          />
        </svg>
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <span className="tabular text-sm font-semibold leading-none">{known ? value : "--"}</span>
          <span className="text-[10px] uppercase tracking-wide text-fg-subtle">%</span>
        </div>
      </div>
      <div className="text-center">
        <div className="text-xs font-medium text-fg">{label}</div>
        <div className="text-[11px] text-fg-subtle">
          {!known ? "Unknown" : charging ? "Charging" : "Discharging"}
        </div>
      </div>
    </div>
  );
}

export function EarbudsStage({
  left,
  right,
  caseCell,
  wornLeft,
  wornRight,
  connected,
  known = true,
}: {
  left: BatteryCell;
  right: BatteryCell;
  caseCell: BatteryCell;
  wornLeft: boolean;
  wornRight: boolean;
  connected: boolean;
  known?: boolean;
}) {
  return (
    <div className="relative overflow-hidden rounded-xl border border-border bg-bg-subtle px-4 py-6 shadow-[var(--shadow-panel)]">
      <div className="relative flex items-end justify-center gap-6 sm:gap-10">
        <Bud side="L" worn={wornLeft} connected={connected} />
        <Case />
        <Bud side="R" worn={wornRight} connected={connected} />
      </div>
      <div className="mt-6 flex justify-center gap-8">
        <Ring value={left.level} charging={left.charging} label="Left" known={known} />
        <Ring value={right.level} charging={right.charging} label="Right" known={known} />
        <Ring value={caseCell.level} charging={caseCell.charging} label="Case" known={known} />
      </div>
    </div>
  );
}

function Bud({ side, worn, connected }: { side: "L" | "R"; worn: boolean; connected: boolean }) {
  const flip = side === "R";
  return (
    <div className={cn("flex flex-col items-center gap-2", !worn && "opacity-50")}>
      <svg
        viewBox="0 0 64 120"
        className={cn("h-28 w-14", flip && "-scale-x-100")}
        aria-hidden
      >
        <ellipse
          cx="32"
          cy="28"
          rx="20"
          ry="24"
          className="fill-bg-elevated stroke-border-strong"
          strokeWidth="1.5"
        />
        <rect
          x="26"
          y="48"
          width="12"
          height="52"
          rx="6"
          className="fill-bg-hover stroke-border-strong"
          strokeWidth="1.5"
        />
        <rect x="28" y="54" width="8" height="18" rx="3" className="fill-accent/80" />
        <circle cx="32" cy="24" r="6" className={connected ? "fill-accent" : "fill-fg-subtle"} />
      </svg>
      <span className="text-[11px] font-medium uppercase tracking-wider text-fg-muted">{side}</span>
    </div>
  );
}

function Case() {
  return (
    <svg viewBox="0 0 88 56" className="mb-6 h-14 w-24" aria-hidden>
      <rect
        x="4"
        y="8"
        width="80"
        height="40"
        rx="20"
        className="fill-bg-elevated stroke-border-strong"
        strokeWidth="1.5"
      />
      <rect x="30" y="6" width="28" height="6" rx="3" className="fill-bg-hover" />
      <circle cx="44" cy="28" r="4" className="fill-accent/70" />
    </svg>
  );
}
