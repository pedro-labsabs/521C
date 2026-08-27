/**
 * Capability truth model (issue #3).
 *
 * A capability is described by four independent truths instead of one ambiguous
 * state, so the UI/CLI/docs cannot confuse "the device/protocol can do this"
 * with "this build implements it":
 *
 *   hardware       – is the feature associated with the hardware/model?
 *   protocol       – is the protocol behavior evidenced for this model/firmware?
 *   implementation – does this build actually implement it?
 *   write          – is the operation writable / experimental / read-only / forbidden?
 *
 * Deterministic rules below derive what is shown, enabled and writable. Host-only
 * features (system EQ, auto game mode, codec status) stay `not-implemented` or
 * `mock-only` until a real host backend exists (issue #13); they are never
 * presented as QCY protocol capabilities.
 */

export type EvidenceTruth = "supported" | "unknown" | "unsupported";
export type ImplTruth = "implemented" | "mock-only" | "not-implemented";
export type WriteReadiness = "writable" | "experimental" | "read-only" | "forbidden";

export type CapabilityTruth = {
  hardware: EvidenceTruth;
  protocol: EvidenceTruth;
  implementation: ImplTruth;
  write: WriteReadiness;
  note?: string;
  /** Evidence-ledger opcode reference when the capability is protocol-backed. */
  opcode?: number;
};

/** Backwards-compatible alias used by existing call sites during migration. */
export type FeatureFlag = CapabilityTruth;

export type DeviceCapabilities = {
  batteryLeft: CapabilityTruth;
  batteryRight: CapabilityTruth;
  batteryCase: CapabilityTruth;
  chargingFlags: CapabilityTruth;
  firmware: CapabilityTruth;
  rssi: CapabilityTruth;
  ancOff: CapabilityTruth;
  ancOn: CapabilityTruth;
  ancAdaptive: CapabilityTruth;
  ancIndoor: CapabilityTruth;
  ancCommuting: CapabilityTruth;
  ancNoisy: CapabilityTruth;
  ancWind: CapabilityTruth;
  ancLevels: CapabilityTruth;
  transparency: CapabilityTruth;
  transparencyLevels: CapabilityTruth;
  vocalEnhance: CapabilityTruth;
  gameMode: CapabilityTruth;
  autoGameMode: CapabilityTruth;
  deviceEq: CapabilityTruth;
  systemEq: CapabilityTruth;
  eqPresets: CapabilityTruth;
  eqCustom: CapabilityTruth;
  eqPerChannel: CapabilityTruth;
  touchControls: CapabilityTruth;
  wearDetection: CapabilityTruth;
  wearAutoPause: CapabilityTruth;
  sleepMode: CapabilityTruth;
  spatialAudio: CapabilityTruth;
  multipointStatus: CapabilityTruth;
  multipointControl: CapabilityTruth;
  findChime: CapabilityTruth;
  findRssi: CapabilityTruth;
  findGps: CapabilityTruth;
  ldacToggle: CapabilityTruth;
  ldacBitrate: CapabilityTruth;
  codecStatus: CapabilityTruth;
  rename: CapabilityTruth;
  firmwareOta: CapabilityTruth;
  inEarSensitivity: CapabilityTruth;
  soundBalance: CapabilityTruth;
  earTipFit: CapabilityTruth;
};

export const HT08_CAPABILITIES: DeviceCapabilities = {
  batteryLeft: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "read-only",
    note: "0x2F / char 00000008",
    opcode: 0x2f,
  },
  batteryRight: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "read-only",
    note: "0x2F / char 00000008",
    opcode: 0x2f,
  },
  batteryCase: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "read-only",
    note: "0x2F / char 00000008",
    opcode: 0x2f,
  },
  chargingFlags: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "read-only",
    note: "bit7 of battery bytes",
    opcode: 0x2f,
  },
  firmware: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "read-only",
    note: "0x30 / char 00000007",
    opcode: 0x30,
  },
  rssi: {
    hardware: "supported",
    protocol: "unknown",
    implementation: "implemented",
    write: "read-only",
    note: "Host BLE RSSI, not GPS",
  },
  ancOff: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x17 payload (2,0,0) validated on live HT08 (notify ACK + audible, 2026-08-27).",
    opcode: 0x17,
  },
  ancOn: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x17 payload (1,1,2) indoor validated on live HT08 (notify ACK + 'ANC on' prompt, 2026-08-27).",
    opcode: 0x17,
  },
  ancAdaptive: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x17 payload (1,5,2) validated on live HT08 (ACK (1,5,0) + 'adaptive' voice prompt, 2026-08-27); 0x32 EnvAdaptation unvalidated on this model.",
    opcode: 0x17,
  },
  ancIndoor: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x17 payload (1,1,2) validated on live HT08; same scene as ancOn on this model.",
    opcode: 0x17,
  },
  ancCommuting: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x17 payload (1,2,2) validated by notify ACK on live HT08 (2026-08-27).",
    opcode: 0x17,
  },
  ancNoisy: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x17 payload (1,3,2) validated by notify ACK on live HT08 (also observed from touch sensor).",
    opcode: 0x17,
  },
  ancWind: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x17 payload (1,4,2) validated on live HT08 (ACK (1,4,0), 2026-08-27).",
    opcode: 0x17,
  },
  ancLevels: {
    hardware: "unknown",
    protocol: "unknown",
    implementation: "not-implemented",
    write: "read-only",
    note: "Not validated on HT08: subScene is the scene selector (1-5) and payloads are fixed per mode; adjustable ANC levels unconfirmed.",
  },
  transparency: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x17 payload (3,2,4) validated on live HT08 (ACK (3,2,0) + audible, 2026-08-27).",
    opcode: 0x17,
  },
  transparencyLevels: {
    hardware: "unknown",
    protocol: "unknown",
    implementation: "not-implemented",
    write: "read-only",
    note: "Not validated on HT08: only transparency (3,2,4) confirmed; subScene level variants unconfirmed.",
  },
  vocalEnhance: {
    hardware: "supported",
    protocol: "unknown",
    implementation: "not-implemented",
    write: "read-only",
    note: "Reviews mention a vocal-enhance transparency mode; not a named opcode in the public table.",
  },
  gameMode: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x09 LowLatency",
    opcode: 0x09,
  },
  autoGameMode: {
    hardware: "unknown",
    protocol: "unknown",
    implementation: "implemented",
    write: "read-only",
    note: "Host automation (qcy-host): MPRIS player-presence signal (session-bus NameOwnerChanged) + debounce + keyword allowlist, no busy polling. Device writes happen only through the central policy once the desktop app (#8) wires the controller; inactive by default, so no BLE traffic while idle. Never written to the buds by the host layer.",
  },
  deviceEq: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x22 / char 0000000B",
    opcode: 0x22,
  },
  systemEq: {
    hardware: "unknown",
    protocol: "unknown",
    implementation: "implemented",
    write: "read-only",
    note: "Host PipeWire EQ (qcy-host): manages one user-scoped config artifact with clear create/remove lifecycle and no system-wide changes. Applying requires a PipeWire reload. Never written to the buds.",
  },
  eqPresets: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "Community band tables written via 0x22",
    opcode: 0x22,
  },
  eqCustom: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x22",
    opcode: 0x22,
  },
  eqPerChannel: {
    hardware: "supported",
    protocol: "unknown",
    implementation: "not-implemented",
    write: "read-only",
    note: "0x46 / 0x47 catalog-only; not enabled.",
    opcode: 0x46,
  },
  touchControls: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "char 0000000D key function V2 (no frame)",
  },
  wearDetection: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x06 / 0x2C",
    opcode: 0x2c,
  },
  wearAutoPause: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x2C musicIndex",
    opcode: 0x2c,
  },
  sleepMode: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x10",
    opcode: 0x10,
  },
  spatialAudio: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "experimental",
    note: "Opcode 0x2D documented; HT08 firmware exposure unverified.",
    opcode: 0x2d,
  },
  multipointStatus: {
    hardware: "unknown",
    protocol: "unknown",
    implementation: "not-implemented",
    write: "read-only",
    note: "A2DP/HFP multipoint is a Bluetooth stack property, not a documented QCY command.",
  },
  multipointControl: {
    hardware: "unknown",
    protocol: "unknown",
    implementation: "not-implemented",
    write: "read-only",
    note: "No public enable/disable or host-list command.",
  },
  findChime: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x05 LightFlash / 0x3D TonePlay; interactive preflight required (#9).",
    opcode: 0x3d,
  },
  findRssi: {
    hardware: "supported",
    protocol: "unknown",
    implementation: "implemented",
    write: "read-only",
    note: "Smoothed host RSSI proximity, not GPS.",
  },
  findGps: {
    hardware: "unsupported",
    protocol: "unsupported",
    implementation: "not-implemented",
    write: "read-only",
    note: "HT08 has no GPS.",
  },
  ldacToggle: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "experimental",
    note: "0x23; on Linux the codec is usually selected by PipeWire/BlueZ, not the earbud opcode.",
    opcode: 0x23,
  },
  ldacBitrate: {
    hardware: "unsupported",
    protocol: "unsupported",
    implementation: "not-implemented",
    write: "read-only",
    note: "BlueZ/PipeWire do not reliably expose LDAC bitrate. Never invented.",
  },
  codecStatus: {
    hardware: "unknown",
    protocol: "unknown",
    implementation: "implemented",
    write: "read-only",
    note: "Read passively from BlueZ MediaTransport1 (codec/profile/sample rate) by qcy-host when a transport is active; fields that cannot be sourced stay unknown — never invented. The web preview shows a placeholder.",
  },
  rename: {
    hardware: "supported",
    protocol: "unknown",
    implementation: "not-implemented",
    write: "read-only",
    note: "0x18 catalog-only; not in the trusted table and no app action yet.",
    opcode: 0x18,
  },
  firmwareOta: {
    hardware: "unsupported",
    protocol: "unsupported",
    implementation: "not-implemented",
    write: "forbidden",
    note: "Firmware update not yet safely supported. No flash/OTA path will be sent.",
  },
  inEarSensitivity: {
    hardware: "supported",
    protocol: "unknown",
    implementation: "not-implemented",
    write: "read-only",
    note: "0x48 catalog-only; not enabled.",
    opcode: 0x48,
  },
  soundBalance: {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    note: "0x16",
    opcode: 0x16,
  },
  earTipFit: {
    hardware: "supported",
    protocol: "unknown",
    implementation: "not-implemented",
    write: "read-only",
    note: "0x11 catalog-only; not enabled.",
    opcode: 0x11,
  },
};

/** Show a capability unless both hardware and protocol say it does not exist. */
export function isShown(cap: CapabilityTruth): boolean {
  return !(cap.hardware === "unsupported" && cap.protocol === "unsupported");
}

/** Implemented in this build (not mock-only, not pending). */
export function isImplemented(cap: CapabilityTruth): boolean {
  return cap.implementation === "implemented";
}

/** A supported write the app implements. */
export function isWritable(cap: CapabilityTruth): boolean {
  return isImplemented(cap) && cap.write === "writable";
}

/** An experimental write the app implements (needs the session opt-in). */
export function isExperimentalWrite(cap: CapabilityTruth): boolean {
  return isImplemented(cap) && cap.write === "experimental";
}

/** The UI may offer interaction: implemented and either writable or experimental. */
export function canInteract(cap: CapabilityTruth): boolean {
  return isImplemented(cap) && (cap.write === "writable" || cap.write === "experimental");
}

export type CapabilitySummary = {
  label: string;
  tone: "supported" | "experimental" | "neutral" | "unknown" | "research" | "danger";
};

/** Honest one-line summary for chips/UI, derived from the four truths. */
export function summarizeCapability(cap: CapabilityTruth): CapabilitySummary {
  if (cap.write === "forbidden") return { label: "Forbidden", tone: "danger" };
  if (cap.implementation === "implemented") {
    if (cap.write === "experimental") return { label: "Experimental", tone: "experimental" };
    if (cap.write === "writable") return { label: "Supported", tone: "supported" };
    return { label: "Read-only", tone: "supported" };
  }
  if (cap.implementation === "mock-only") return { label: "Mock only", tone: "neutral" };
  // not-implemented below here
  if (cap.protocol === "supported") return { label: "Protocol known \u00b7 app pending", tone: "neutral" };
  if (cap.hardware === "unsupported" && cap.protocol === "unsupported") {
    return { label: "Unsupported", tone: "neutral" };
  }
  if (cap.hardware === "supported") {
    return { label: "Needs protocol research", tone: "research" };
  }
  return { label: "Unknown", tone: "unknown" };
}
