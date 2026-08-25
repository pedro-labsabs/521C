import { summarizeCapability, type CapabilityTruth } from "@/lib/qcy/device/capabilities";
import { cn } from "@/lib/utils";

/**
 * Renders an honest one-line summary of a capability's four truths
 * (hardware / protocol / implementation / write). See capabilities.ts.
 */
export function CapabilityChip({ cap }: { cap: CapabilityTruth }) {
  const { label, tone } = summarizeCapability(cap);
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
        tone === "supported" && "bg-accent/15 text-accent",
        tone === "experimental" && "bg-warn/15 text-warn",
        tone === "neutral" && "bg-bg-hover text-fg-subtle",
        tone === "unknown" && "bg-bg-hover text-fg-muted",
        tone === "research" && "bg-danger/10 text-danger",
        tone === "danger" && "bg-danger/15 text-danger",
      )}
    >
      {label}
    </span>
  );
}
