import type { CapabilityState } from "@/lib/qcy/protocol/types";
import { cn } from "@/lib/utils";

const LABELS: Record<CapabilityState, string> = {
  supported: "Supported",
  unsupported: "Unsupported",
  experimental: "Experimental",
  unknown: "Unknown",
  "requires-protocol-research": "Needs research",
};

export function CapabilityChip({ state }: { state: CapabilityState }) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
        state === "supported" && "bg-accent/15 text-accent",
        state === "experimental" && "bg-warn/15 text-warn",
        state === "unsupported" && "bg-bg-hover text-fg-subtle",
        state === "unknown" && "bg-bg-hover text-fg-muted",
        state === "requires-protocol-research" && "bg-danger/10 text-danger",
      )}
    >
      {LABELS[state]}
    </span>
  );
}
