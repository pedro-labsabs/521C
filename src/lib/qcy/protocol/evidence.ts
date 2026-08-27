import { CHAR, SERVICE, STD } from "./uuids";

/**
 * Canonical protocol evidence ledger.
 *
 * Every opcode that remains in the repository is recorded here with its
 * provenance (evidence class), the device/model it was observed on, its
 * read/write status, payload confidence, and the 521C trust level that the
 * central write policy derives. This is the single source of truth for which
 * commands may become writable — an opcode existing in `Cmd` is NOT by itself
 * sufficient to write it. See docs/PROTOCOL.md and docs/SECURITY_MODEL.md.
 *
 * Evidence classes preserve uncertainty: `community-catalog` and `official-app`
 * entries are research leads, not proof, and stay non-writable until elevated by
 * real evidence (issue #6 governance).
 */

export type EvidenceClass =
  | "protocol-doc"
  | "community-catalog"
  | "official-app"
  | "hardware-capture";

export type TrustLevel =
  | "write-supported"
  | "write-experimental"
  | "read"
  | "catalog-only"
  | "destructive";

export type PayloadConfidence = "high" | "medium" | "low" | "unknown";

export type OpcodeEvidence = {
  opcode: number;
  name: string;
  /** Where this fact came from. */
  evidenceClass: EvidenceClass;
  /** 521C trust level governing writability. */
  trustLevel: TrustLevel;
  /** Model the behavior was observed/documented on, if known. */
  observedModel: string | null;
  /** Firmware version observed, if known. */
  firmware: string | null;
  readWrite: "read" | "write" | "read-write";
  payloadConfidence: PayloadConfidence;
  /** Capability key this opcode maps to, when one exists. */
  capability?: string;
  notes?: string;
};

export const OPCODE_EVIDENCE: ReadonlyMap<number, OpcodeEvidence> = new Map([
  [
    0x01,
    {
      opcode: 0x01,
      name: "ResetDefault",
      evidenceClass: "protocol-doc",
      trustLevel: "destructive",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "high",
    notes: "Documented destructive reset. Never automated.",
    },
  ],
  [
    0x02,
    {
      opcode: 0x02,
      name: "ClearPairing",
      evidenceClass: "protocol-doc",
      trustLevel: "destructive",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "high",
    notes: "Documented destructive pairing clear. Never automated.",
    },
  ],
  [
    0x03,
    {
      opcode: 0x03,
      name: "FactoryReset",
      evidenceClass: "protocol-doc",
      trustLevel: "destructive",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "high",
    notes: "Documented destructive factory reset. Never automated.",
    },
  ],
  [
    0x04,
    {
      opcode: 0x04,
      name: "MusicControl",
      evidenceClass: "official-app",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "low",
    notes: "Official-app media control; not in trusted table. Host MPRIS path tracked by #13.",
    },
  ],
  [
    0x05,
    {
      opcode: 0x05,
      name: "LightFlash",
      evidenceClass: "protocol-doc",
      trustLevel: "write-supported",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "high",
    capability: "findChime",
    notes: "Find-earbuds LED flash.",
    },
  ],
  [
    0x06,
    {
      opcode: 0x06,
      name: "InEarDetection",
      evidenceClass: "protocol-doc",
      trustLevel: "write-supported",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "high",
    capability: "wearDetection",
    notes: "In-ear detection enable/disable.",
    },
  ],
  [
    0x07,
    {
      opcode: 0x07,
      name: "NoiseValue",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x08,
    {
      opcode: 0x08,
      name: "Volume",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x09,
    {
      opcode: 0x09,
      name: "LowLatency",
      evidenceClass: "protocol-doc",
      trustLevel: "write-supported",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "high",
    capability: "gameMode",
    notes: "Game / low-latency mode.",
    },
  ],
  [
    0x0a,
    {
      opcode: 0x0a,
      name: "Monitoring",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "read",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x0c,
    {
      opcode: 0x0c,
      name: "NoiseCancelMode",
      evidenceClass: "protocol-doc",
      trustLevel: "write-experimental",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "low",
    capability: "ancOn",
    notes:
      "Simple noise-cancel mode (off/ANC/outdoor/transparency). Live HT08 test unit ignored 0x0C writes on 2026-08-27 (no ACK, no audible effect) while 0x17 AncSetting writes executed; demoted pending cross-unit evidence.",
    },
  ],
  [
    0x0d,
    {
      opcode: 0x0d,
      name: "TestMode",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    notes: "Diagnostic/test; not a user write.",
    },
  ],
  [
    0x10,
    {
      opcode: 0x10,
      name: "SleepMode",
      evidenceClass: "protocol-doc",
      trustLevel: "write-supported",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "high",
    capability: "sleepMode",
    notes: "Sleep mode enable/disable.",
    },
  ],
  [
    0x11,
    {
      opcode: 0x11,
      name: "EarTipFit",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "low",
    capability: "earTipFit",
    notes: "Capability matrix lists experimental; not in trusted doc; not enabled.",
    },
  ],
  [
    0x12,
    {
      opcode: 0x12,
      name: "LedMode",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x14,
    {
      opcode: 0x14,
      name: "PowerManager",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x16,
    {
      opcode: 0x16,
      name: "SoundBalance",
      evidenceClass: "protocol-doc",
      trustLevel: "write-supported",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "high",
    capability: "soundBalance",
    notes: "L/R sound balance 0-100, 50 center.",
    },
  ],
  [
    0x17,
    {
      opcode: 0x17,
      name: "AncSetting",
      evidenceClass: "hardware-capture",
      trustLevel: "write-supported",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "high",
    capability: "ancLevels",
    notes:
      "ANC scene: mode, subScene, noiseValue. Confirmed on live HT08 over BLE GATT 2026-08-27: writes to char 00001001 of service 0000a001 executed with audible effect and notify ACK echoing the resulting state. Confirmed payloads: off=(2,0,0), indoor/ANC=(1,1,2), transparency=(3,2,4) ACKed as (3,2,0). Touch-sensor changes produced (1,3,2)=noisy and (2,0,0)=off. Cross-checked with OpenQCY mode table: commuting=(1,2,2), noisy=(1,3,2), wind=(1,4,2), adaptive=(1,5,2) (individual confirmation pending).",
    },
  ],
  [
    0x18,
    {
      opcode: 0x18,
      name: "RenameDevice",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "low",
    capability: "rename",
    notes: "Capability matrix lists supported; not in trusted doc; not enabled pending evidence.",
    },
  ],
  [
    0x19,
    {
      opcode: 0x19,
      name: "VoiceLanguage",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x1d,
    {
      opcode: 0x1d,
      name: "ToneVolume",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x1e,
    {
      opcode: 0x1e,
      name: "TakePhoto",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x1f,
    {
      opcode: 0x1f,
      name: "Standby",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x20,
    {
      opcode: 0x20,
      name: "EqParamsV1",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "low",
    notes: "Superseded by EqParamsV2 (0x22).",
    },
  ],
  [
    0x22,
    {
      opcode: 0x22,
      name: "EqParamsV2",
      evidenceClass: "protocol-doc",
      trustLevel: "write-supported",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "high",
    capability: "deviceEq",
    notes: "Parametric device EQ bands.",
    },
  ],
  [
    0x23,
    {
      opcode: 0x23,
      name: "Ldac",
      evidenceClass: "protocol-doc",
      trustLevel: "write-experimental",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "medium",
    capability: "ldacToggle",
    notes: "Experimental on Linux; codec usually selected by PipeWire/BlueZ.",
    },
  ],
  [
    0x27,
    {
      opcode: 0x27,
      name: "AdaptiveEq",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x28,
    {
      opcode: 0x28,
      name: "AncResult",
      evidenceClass: "hardware-capture",
      trustLevel: "read",
      observedModel: "HT08",
      firmware: null,
      readWrite: "read",
      payloadConfidence: "medium",
    },
  ],
  [
    0x29,
    {
      opcode: 0x29,
      name: "AncWear",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "read",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x2b,
    {
      opcode: 0x2b,
      name: "KeyFunction",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "low",
    capability: "touchControls",
    notes: "Touch mapping uses direct char 0000000D (no framing), not this framed opcode.",
    },
  ],
  [
    0x2c,
    {
      opcode: 0x2c,
      name: "WearingDetection",
      evidenceClass: "protocol-doc",
      trustLevel: "write-supported",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "high",
    capability: "wearDetection",
    notes: "Wearing detection settings.",
    },
  ],
  [
    0x2d,
    {
      opcode: 0x2d,
      name: "SpatialAudio",
      evidenceClass: "protocol-doc",
      trustLevel: "write-experimental",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "medium",
    capability: "spatialAudio",
    notes: "Opcode documented; HT08 firmware exposure unverified.",
    },
  ],
  [
    0x2e,
    {
      opcode: 0x2e,
      name: "MusicMode",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x2f,
    {
      opcode: 0x2f,
      name: "Battery",
      evidenceClass: "protocol-doc",
      trustLevel: "read",
      observedModel: "HT08",
      firmware: null,
      readWrite: "read",
      payloadConfidence: "high",
    capability: "batteryLeft",
    notes: "Battery L/R/case + charging flags.",
    },
  ],
  [
    0x30,
    {
      opcode: 0x30,
      name: "Version",
      evidenceClass: "protocol-doc",
      trustLevel: "read",
      observedModel: "HT08",
      firmware: null,
      readWrite: "read",
      payloadConfidence: "high",
    capability: "firmware",
    notes: "Firmware version readout.",
    },
  ],
  [
    0x32,
    {
      opcode: 0x32,
      name: "EnvAdaptation",
      evidenceClass: "protocol-doc",
      trustLevel: "write-experimental",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "medium",
    capability: "ancAdaptive",
    notes: "Adaptive ANC mapping experimental until captured on-device.",
    },
  ],
  [
    0x34,
    {
      opcode: 0x34,
      name: "TwsEnable",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x35,
    {
      opcode: 0x35,
      name: "LedSwitch",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x36,
    {
      opcode: 0x36,
      name: "LedEffect",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x37,
    {
      opcode: 0x37,
      name: "PlayMode",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x39,
    {
      opcode: 0x39,
      name: "FocusMode",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x3a,
    {
      opcode: 0x3a,
      name: "MusicStatus",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "read",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x3b,
    {
      opcode: 0x3b,
      name: "MusicInfo",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "read",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x3d,
    {
      opcode: 0x3d,
      name: "TonePlay",
      evidenceClass: "protocol-doc",
      trustLevel: "write-supported",
      observedModel: "HT08",
      firmware: null,
      readWrite: "write",
      payloadConfidence: "high",
    capability: "findChime",
    notes: "Locator chime tone.",
    },
  ],
  [
    0x3e,
    {
      opcode: 0x3e,
      name: "SyncTime",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x3f,
    {
      opcode: 0x3f,
      name: "Alarm",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x43,
    {
      opcode: 0x43,
      name: "Ai",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x44,
    {
      opcode: 0x44,
      name: "MaxEqCount",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "read",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x45,
    {
      opcode: 0x45,
      name: "CustomEqTest",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0x46,
    {
      opcode: 0x46,
      name: "EqLeft",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "low",
    capability: "eqPerChannel",
    notes: "Capability matrix lists experimental; not enabled.",
    },
  ],
  [
    0x47,
    {
      opcode: 0x47,
      name: "EqRight",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "low",
    capability: "eqPerChannel",
    notes: "Capability matrix lists experimental; not enabled.",
    },
  ],
  [
    0x48,
    {
      opcode: 0x48,
      name: "InEarSensitivity",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "low",
    capability: "inEarSensitivity",
    notes: "Capability matrix lists experimental; not enabled.",
    },
  ],
  [
    0x4a,
    {
      opcode: 0x4a,
      name: "GameConfig",
      evidenceClass: "community-catalog",
      trustLevel: "catalog-only",
      observedModel: null,
      firmware: null,
      readWrite: "write",
      payloadConfidence: "unknown",
    },
  ],
  [
    0xfe,
    {
      opcode: 0xfe,
      name: "RequestData",
      evidenceClass: "protocol-doc",
      trustLevel: "read",
      observedModel: "HT08",
      firmware: null,
      readWrite: "read",
      payloadConfidence: "high",
    notes: "Read-back request wrapper for another opcode.",
    },
  ],
]);

/** Opcodes the connected HT08 profile may write without opt-in. */
export function supportedWriteOpcodes(): Set<number> {
  const out = new Set<number>();
  for (const [op, e] of OPCODE_EVIDENCE) {
    if (e.trustLevel === "write-supported") out.add(op);
  }
  return out;
}

/** Opcodes that are experimental and require a session opt-in to enable. */
export function experimentalWriteOpcodes(): Set<number> {
  const out = new Set<number>();
  for (const [op, e] of OPCODE_EVIDENCE) {
    if (e.trustLevel === "write-experimental") out.add(op);
  }
  return out;
}

/** Opcodes that are destructive and must never be written. */
export function destructiveOpcodes(): Set<number> {
  const out = new Set<number>();
  for (const [op, e] of OPCODE_EVIDENCE) {
    if (e.trustLevel === "destructive") out.add(op);
  }
  return out;
}

export type UuidRole =
  | "service"
  | "framed-write"
  | "notify"
  | "read"
  | "direct-write"
  | "standard"
  | "unused";

export type UuidEvidence = {
  uuid: string;
  name: string;
  evidenceClass: EvidenceClass;
  role: UuidRole;
  observedModel: string | null;
  notes?: string;
};

/**
 * GATT UUID evidence. Direct-write characteristics are the only unframed write
 * surface; everything else here is read/notify/unused or Bluetooth-SIG standard.
 */
export const UUID_EVIDENCE: ReadonlyMap<string, UuidEvidence> = new Map([
  [SERVICE.main, { uuid: SERVICE.main, name: "Main service", evidenceClass: "protocol-doc", role: "service", observedModel: "HT08" }],
  [CHAR.commandWrite, { uuid: CHAR.commandWrite, name: "Command write (0xFF framed)", evidenceClass: "protocol-doc", role: "framed-write", observedModel: "HT08" }],
  [CHAR.settingsNotify, { uuid: CHAR.settingsNotify, name: "Notify / settings", evidenceClass: "protocol-doc", role: "notify", observedModel: "HT08" }],
  [CHAR.version, { uuid: CHAR.version, name: "Version read", evidenceClass: "protocol-doc", role: "read", observedModel: "HT08" }],
  [CHAR.battery, { uuid: CHAR.battery, name: "Battery read", evidenceClass: "protocol-doc", role: "read", observedModel: "HT08" }],
  [CHAR.eqDirect, { uuid: CHAR.eqDirect, name: "EQ direct (no frame)", evidenceClass: "protocol-doc", role: "direct-write", observedModel: "HT08" }],
  [CHAR.keyFunctionV2, { uuid: CHAR.keyFunctionV2, name: "Key function V2 (no frame)", evidenceClass: "protocol-doc", role: "direct-write", observedModel: "HT08" }],
  [CHAR.language, { uuid: CHAR.language, name: "Language", evidenceClass: "community-catalog", role: "unused", observedModel: null }],
  [CHAR.resetV1, { uuid: CHAR.resetV1, name: "Reset V1", evidenceClass: "community-catalog", role: "unused", observedModel: null, notes: "Destructive surface; never used." }],
  [CHAR.sendTimeV1, { uuid: CHAR.sendTimeV1, name: "Send time V1", evidenceClass: "community-catalog", role: "unused", observedModel: null }],
  [CHAR.zrSettings, { uuid: CHAR.zrSettings, name: "ZR settings", evidenceClass: "community-catalog", role: "unused", observedModel: null }],
  [CHAR.inEarCheckJl, { uuid: CHAR.inEarCheckJl, name: "In-ear check JL", evidenceClass: "community-catalog", role: "unused", observedModel: null }],
  [CHAR.unknown1003, { uuid: CHAR.unknown1003, name: "Unknown 1003", evidenceClass: "community-catalog", role: "unused", observedModel: null }],
  [CHAR.leftSingleTapV1, { uuid: CHAR.leftSingleTapV1, name: "Left single tap V1", evidenceClass: "community-catalog", role: "unused", observedModel: null }],
  [CHAR.rightSingleTapV1, { uuid: CHAR.rightSingleTapV1, name: "Right single tap V1", evidenceClass: "community-catalog", role: "unused", observedModel: null }],
  [CHAR.leftDoubleTapV1, { uuid: CHAR.leftDoubleTapV1, name: "Left double tap V1", evidenceClass: "community-catalog", role: "unused", observedModel: null }],
  [CHAR.rightDoubleTapV1, { uuid: CHAR.rightDoubleTapV1, name: "Right double tap V1", evidenceClass: "community-catalog", role: "unused", observedModel: null }],
  [CHAR.leftTripleTapV1, { uuid: CHAR.leftTripleTapV1, name: "Left triple tap V1", evidenceClass: "community-catalog", role: "unused", observedModel: null }],
  [CHAR.rightTripleTapV1, { uuid: CHAR.rightTripleTapV1, name: "Right triple tap V1", evidenceClass: "community-catalog", role: "unused", observedModel: null }],
  [STD.batteryLevel, { uuid: STD.batteryLevel, name: "Battery Level (SIG)", evidenceClass: "protocol-doc", role: "standard", observedModel: null }],
  [STD.modelNumber, { uuid: STD.modelNumber, name: "Model Number (SIG)", evidenceClass: "protocol-doc", role: "standard", observedModel: null }],
  [STD.firmwareRevision, { uuid: STD.firmwareRevision, name: "Firmware Revision (SIG)", evidenceClass: "protocol-doc", role: "standard", observedModel: null }],
  [STD.manufacturerName, { uuid: STD.manufacturerName, name: "Manufacturer Name (SIG)", evidenceClass: "protocol-doc", role: "standard", observedModel: null }],
  [STD.pnpId, { uuid: STD.pnpId, name: "PnP ID (SIG)", evidenceClass: "protocol-doc", role: "standard", observedModel: null }],
]);

/** Characteristics allowed for unframed direct writes. */
export function directWriteUuids(): Set<string> {
  const out = new Set<string>();
  for (const [uuid, e] of UUID_EVIDENCE) {
    if (e.role === "direct-write") out.add(uuid);
  }
  return out;
}
