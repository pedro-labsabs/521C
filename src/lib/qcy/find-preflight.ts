/**
 * Find Earbuds chime preflight safety gate (issue #9).
 *
 * Audible locator actions must require deliberate user intent before any tone is
 * emitted. This module is pure (no transport, no I/O): it turns the requested side
 * plus the currently observed wear evidence into a preflight decision. The store
 * only transmits after an interactive confirmation completes, and never when the
 * decision is `blocked-worn`.
 *
 * Wear evidence is used conservatively: absence of a "worn" signal is NOT treated
 * as "safe". When wear detection is disabled (state unknown/stale) the decision
 * escalates to `confirm-strong` instead of assuming the bud is out of the ear.
 */

export type ChimeSide = "left" | "right" | "both";
export type ChimeTarget = "left" | "right";

/**
 * - `blocked-worn`: a target bud is known to be worn/in-ear -> block by default.
 * - `confirm-strong`: wear state unknown/stale -> require an explicit, stronger
 *   confirmation rather than assuming the bud is out of the ear.
 * - `confirm`: targets known not worn -> standard confirmation with warning.
 */
export type PreflightStatus = "blocked-worn" | "confirm-strong" | "confirm";

export type WearEvidence = {
  /** Wear/in-ear detection enabled. When false, worn state is unreliable. */
  detectionEnabled: boolean;
  /**
   * Null = never observed (real sessions start unknown, issue #62). Unknown
   * is treated like a missing signal: it escalates to confirm-strong and is
   * never read as "not worn".
   */
  wornLeft: boolean | null;
  wornRight: boolean | null;
};

export type ChimePreflight = {
  side: ChimeSide;
  targets: ChimeTarget[];
  status: PreflightStatus;
  wornTargets: ChimeTarget[];
  unknownTargets: ChimeTarget[];
  notWornTargets: ChimeTarget[];
  /** Human-readable explanation shown before emission. */
  reason: string;
};

/** Short cooldown to stop accidental repeated chimes (double-click / automation). */
export const CHIME_COOLDOWN_MS = 5000;

/** Tone id placed on the wire for each side (existing documented mapping). */
export function chimeToneId(side: ChimeSide): number {
  return side === "left" ? 1 : side === "right" ? 2 : 3;
}

function labelTargets(targets: ChimeTarget[]): string {
  return targets.map((t) => (t === "left" ? "the left bud" : "the right bud")).join(" and ");
}

export function evaluateChimePreflight(side: ChimeSide, wear: WearEvidence): ChimePreflight {
  const targets: ChimeTarget[] = side === "both" ? ["left", "right"] : [side];
  const wornTargets: ChimeTarget[] = [];
  const unknownTargets: ChimeTarget[] = [];
  const notWornTargets: ChimeTarget[] = [];

  for (const t of targets) {
    const worn = t === "left" ? wear.wornLeft : wear.wornRight;
    if (!wear.detectionEnabled || worn === null) unknownTargets.push(t);
    else if (worn) wornTargets.push(t);
    else notWornTargets.push(t);
  }

  let status: PreflightStatus;
  let reason: string;
  if (wornTargets.length > 0) {
    status = "blocked-worn";
    const subj = labelTargets(wornTargets);
    reason = `${subj[0]!.toUpperCase()}${subj.slice(1)} ${wornTargets.length === 1 ? "is" : "are"} currently worn / in-ear. Remove ${wornTargets.length === 1 ? "it" : "them"} from your ear before playing the locator tone.`;
  } else if (unknownTargets.length > 0) {
    status = "confirm-strong";
    reason = `Wear state for ${labelTargets(unknownTargets)} is unknown (wear detection is off or the last reading is stale). Confirm the bud is not in your ear before playing the tone.`;
  } else {
    status = "confirm";
    reason = `${labelTargets(notWornTargets)[0]!.toUpperCase()}${labelTargets(notWornTargets).slice(1)} ${notWornTargets.length === 1 ? "is" : "are"} not worn. Confirm before playing the locator tone.`;
  }

  return { side, targets, status, wornTargets, unknownTargets, notWornTargets, reason };
}
