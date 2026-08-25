import type { DeviceCapabilities } from "./capabilities";
import { HT08_CAPABILITIES } from "./capabilities";
import {
  directWriteUuids,
  experimentalWriteOpcodes,
  supportedWriteOpcodes,
} from "../protocol/evidence";

/**
 * Per-profile write authorization surface consumed by the central policy in
 * `src/lib/qcy/policy.ts`. Membership must follow the evidence model: only
 * opcodes/characteristics with sufficient support for this model belong here.
 * Issue #6 enriches this with explicit provenance; until then it mirrors the
 * documented trusted write surface, not the full community catalog.
 */
export type DeviceWritePolicy = {
  /** Framed opcodes allowed as supported state-changing writes. */
  supportedOpcodes: ReadonlySet<number>;
  /** Framed opcodes that are experimental and need a session opt-in to enable. */
  experimentalOpcodes: ReadonlySet<number>;
  /** Unframed direct-write characteristics allowed for this profile. */
  directChars: ReadonlySet<string>;
};

export type QcyDeviceProfile = {
  id: string;
  title: string;
  subtitle: string;
  matchNames: string[];
  /** Confirmed vendorIds from manufacturer data. Empty until captured. */
  vendorIds: number[];
  bluetooth: string;
  drivers: string;
  mics: string;
  codecs: string[];
  capabilities: DeviceCapabilities;
  /** Unknown/generic devices are read-only by default. */
  readOnly: boolean;
  writePolicy: DeviceWritePolicy;
};

/**
 * HT08 trusted write surface, derived from the canonical evidence ledger
 * (`src/lib/qcy/protocol/evidence.ts`). An opcode is writable only if the ledger
 * records it as `write-supported` (or `write-experimental` behind a session
 * opt-in). Opcodes present in the community catalog but not evidenced for HT08
 * (e.g. MusicControl 0x04, Volume 0x08, NoiseValue 0x07, Rename 0x18) stay
 * `catalog-only` and are therefore not writable.
 */
const HT08_WRITE_POLICY: DeviceWritePolicy = {
  supportedOpcodes: supportedWriteOpcodes(),
  experimentalOpcodes: experimentalWriteOpcodes(),
  directChars: directWriteUuids(),
};

export const HT08_PROFILE: QcyDeviceProfile = {
  id: "HT08",
  title: "QCY MeloBuds Pro",
  subtitle: "QCY-HT08",
  matchNames: ["QCY MeloBuds Pro", "MeloBuds Pro", "QCY HT08", "HT08", "QCY-HT08"],
  vendorIds: [],
  bluetooth: "5.3",
  drivers: "12 mm bio-diaphragm",
  mics: "6 microphones",
  codecs: ["SBC", "AAC", "LDAC"],
  capabilities: HT08_CAPABILITIES,
  readOnly: false,
  writePolicy: HT08_WRITE_POLICY,
};

const READ_ONLY_WRITE_POLICY: DeviceWritePolicy = {
  supportedOpcodes: new Set<number>(),
  experimentalOpcodes: new Set<number>(),
  directChars: new Set<string>(),
};

export const GENERIC_QCY_PROFILE: QcyDeviceProfile = {
  id: "GENERIC",
  title: "QCY earphones",
  subtitle: "Unknown model",
  matchNames: ["QCY"],
  vendorIds: [],
  bluetooth: "unknown",
  drivers: "unknown",
  mics: "unknown",
  codecs: ["SBC"],
  capabilities: Object.fromEntries(
    Object.entries(HT08_CAPABILITIES).map(([k, v]) => [
      k,
      k.startsWith("battery") || k === "firmware" || k === "rssi"
        ? v
        : {
            hardware: "unknown" as const,
            protocol: "unknown" as const,
            implementation: "not-implemented" as const,
            write: "read-only" as const,
            note: "Identify the model before enabling writes.",
          },
    ]),
  ) as DeviceCapabilities,
  readOnly: true,
  writePolicy: READ_ONLY_WRITE_POLICY,
};

const PROFILES: QcyDeviceProfile[] = [HT08_PROFILE, GENERIC_QCY_PROFILE];

export function identifyProfile(input: {
  name?: string;
  vendorId?: number;
}): QcyDeviceProfile {
  const name = (input.name ?? "").toLowerCase();
  if (input.vendorId !== undefined) {
    const byVendor = PROFILES.find((p) => p.vendorIds.includes(input.vendorId!));
    if (byVendor) return byVendor;
  }
  for (const profile of PROFILES) {
    if (profile.id === "GENERIC") continue;
    if (profile.matchNames.some((n) => name.includes(n.toLowerCase()))) {
      return profile;
    }
  }
  if (name.includes("qcy")) return GENERIC_QCY_PROFILE;
  return GENERIC_QCY_PROFILE;
}

export function allProfiles(): QcyDeviceProfile[] {
  return PROFILES;
}
