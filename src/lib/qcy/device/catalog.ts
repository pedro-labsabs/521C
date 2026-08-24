import type { DeviceCapabilities } from "./capabilities";
import { HT08_CAPABILITIES } from "./capabilities";

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
        : { state: "unknown" as const, note: "Identify the model before enabling writes." },
    ]),
  ) as DeviceCapabilities,
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
