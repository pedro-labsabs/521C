import type { CapabilityState } from "../protocol/types";

export type FeatureFlag = {
  state: CapabilityState;
  note?: string;
  protocol?: string;
};

export type DeviceCapabilities = {
  batteryLeft: FeatureFlag;
  batteryRight: FeatureFlag;
  batteryCase: FeatureFlag;
  chargingFlags: FeatureFlag;
  firmware: FeatureFlag;
  rssi: FeatureFlag;
  ancOff: FeatureFlag;
  ancOn: FeatureFlag;
  ancAdaptive: FeatureFlag;
  ancIndoor: FeatureFlag;
  ancCommuting: FeatureFlag;
  ancNoisy: FeatureFlag;
  ancWind: FeatureFlag;
  ancLevels: FeatureFlag;
  transparency: FeatureFlag;
  transparencyLevels: FeatureFlag;
  vocalEnhance: FeatureFlag;
  gameMode: FeatureFlag;
  autoGameMode: FeatureFlag;
  deviceEq: FeatureFlag;
  systemEq: FeatureFlag;
  eqPresets: FeatureFlag;
  eqCustom: FeatureFlag;
  eqPerChannel: FeatureFlag;
  touchControls: FeatureFlag;
  wearDetection: FeatureFlag;
  wearAutoPause: FeatureFlag;
  sleepMode: FeatureFlag;
  spatialAudio: FeatureFlag;
  multipointStatus: FeatureFlag;
  multipointControl: FeatureFlag;
  findChime: FeatureFlag;
  findRssi: FeatureFlag;
  findGps: FeatureFlag;
  ldacToggle: FeatureFlag;
  ldacBitrate: FeatureFlag;
  codecStatus: FeatureFlag;
  rename: FeatureFlag;
  firmwareOta: FeatureFlag;
  inEarSensitivity: FeatureFlag;
  soundBalance: FeatureFlag;
  earTipFit: FeatureFlag;
};

export const HT08_CAPABILITIES: DeviceCapabilities = {
  batteryLeft: { state: "supported", protocol: "0x2F / char 00000008" },
  batteryRight: { state: "supported", protocol: "0x2F / char 00000008" },
  batteryCase: { state: "supported", protocol: "0x2F / char 00000008" },
  chargingFlags: { state: "supported", protocol: "bit7 of battery bytes" },
  firmware: { state: "supported", protocol: "0x30 / char 00000007" },
  rssi: { state: "supported", note: "Host BLE RSSI, not GPS" },
  ancOff: { state: "supported", protocol: "0x0C mode 0x00 / 0x17" },
  ancOn: { state: "supported", protocol: "0x0C mode 0x01" },
  ancAdaptive: {
    state: "experimental",
    protocol: "0x32 EnvAdaptation + ANC on",
    note: "HT08 hardware advertises Adaptive ANC; mapping onto 0x32 is experimental until captured on-device.",
  },
  ancIndoor: { state: "supported", protocol: "0x17 mode 0x02 sub 1–3 (silent)" },
  ancCommuting: { state: "supported", protocol: "0x17 mode 0x03 sub 1–3 (working)" },
  ancNoisy: { state: "supported", protocol: "0x17 mode 0x04 sub 1–3" },
  ancWind: {
    state: "requires-protocol-research",
    note: "Present in the official app; no public opcode isolated yet.",
  },
  ancLevels: { state: "supported", protocol: "0x17 subScene 1–3" },
  transparency: { state: "supported", protocol: "0x0C mode 0x03 / 0x17 mode 0x0A" },
  transparencyLevels: { state: "supported", protocol: "0x17 mode 0x0A sub 1–7" },
  vocalEnhance: {
    state: "experimental",
    note: "Reviews mention a vocal-enhance transparency mode; not a named opcode in the public table.",
  },
  gameMode: { state: "supported", protocol: "0x09 LowLatency" },
  autoGameMode: { state: "supported", note: "Host-side, no extra BLE traffic while idle" },
  deviceEq: { state: "supported", protocol: "0x22 / char 0000000B" },
  systemEq: { state: "supported", note: "Host PipeWire-style EQ — never written to the buds" },
  eqPresets: { state: "supported", note: "Community band tables written via 0x22" },
  eqCustom: { state: "supported", protocol: "0x22" },
  eqPerChannel: { state: "experimental", protocol: "0x46 / 0x47" },
  touchControls: { state: "supported", protocol: "0x2B / char 0000000D" },
  wearDetection: { state: "supported", protocol: "0x06 / 0x2C" },
  wearAutoPause: { state: "supported", protocol: "0x2C musicIndex" },
  sleepMode: { state: "supported", protocol: "0x10" },
  spatialAudio: {
    state: "experimental",
    protocol: "0x2D",
    note: "Opcode exists; HT08 firmware exposure is unverified.",
  },
  multipointStatus: {
    state: "unknown",
    note: "A2DP/HFP multipoint is a Bluetooth stack property, not a documented QCY command.",
  },
  multipointControl: {
    state: "requires-protocol-research",
    note: "No public enable/disable or host-list command.",
  },
  findChime: { state: "supported", protocol: "0x05 LightFlash / 0x3D TonePlay" },
  findRssi: { state: "supported", note: "Smoothed host RSSI proximity, not GPS" },
  findGps: { state: "unsupported", note: "HT08 has no GPS" },
  ldacToggle: {
    state: "experimental",
    protocol: "0x23",
    note: "On Linux, codec is usually selected by PipeWire/BlueZ, not the earbud opcode.",
  },
  ldacBitrate: {
    state: "unsupported",
    note: "BlueZ/PipeWire do not reliably expose LDAC bitrate. Never invented.",
  },
  codecStatus: { state: "supported", note: "Read from host audio graph when available; mocked here" },
  rename: { state: "supported", protocol: "0x18" },
  firmwareOta: {
    state: "unsupported",
    note: "Firmware update: not yet safely supported. No flash/OTA path will be sent.",
  },
  inEarSensitivity: { state: "experimental", protocol: "0x48" },
  soundBalance: { state: "supported", protocol: "0x16" },
  earTipFit: { state: "experimental", protocol: "0x11" },
};

export function isShown(flag: FeatureFlag): boolean {
  return flag.state === "supported" || flag.state === "experimental";
}

export function isWritable(flag: FeatureFlag): boolean {
  return flag.state === "supported" || flag.state === "experimental";
}
