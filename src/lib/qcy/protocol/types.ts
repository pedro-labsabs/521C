export const SOF = 0xff;

export type CmdId = number;

export const Cmd = {
  ResetDefault: 0x01,
  ClearPairing: 0x02,
  FactoryReset: 0x03,
  MusicControl: 0x04,
  LightFlash: 0x05,
  InEarDetection: 0x06,
  NoiseValue: 0x07,
  Volume: 0x08,
  LowLatency: 0x09,
  Monitoring: 0x0a,
  NoiseCancelMode: 0x0c,
  TestMode: 0x0d,
  SleepMode: 0x10,
  EarTipFit: 0x11,
  LedMode: 0x12,
  PowerManager: 0x14,
  SoundBalance: 0x16,
  AncSetting: 0x17,
  RenameDevice: 0x18,
  VoiceLanguage: 0x19,
  ToneVolume: 0x1d,
  TakePhoto: 0x1e,
  Standby: 0x1f,
  EqParamsV1: 0x20,
  EqParamsV2: 0x22,
  Ldac: 0x23,
  AdaptiveEq: 0x27,
  AncResult: 0x28,
  AncWear: 0x29,
  KeyFunction: 0x2b,
  WearingDetection: 0x2c,
  SpatialAudio: 0x2d,
  MusicMode: 0x2e,
  Battery: 0x2f,
  Version: 0x30,
  EnvAdaptation: 0x32,
  TwsEnable: 0x34,
  LedSwitch: 0x35,
  LedEffect: 0x36,
  PlayMode: 0x37,
  FocusMode: 0x39,
  MusicStatus: 0x3a,
  MusicInfo: 0x3b,
  TonePlay: 0x3d,
  SyncTime: 0x3e,
  Alarm: 0x3f,
  Ai: 0x43,
  MaxEqCount: 0x44,
  CustomEqTest: 0x45,
  EqLeft: 0x46,
  EqRight: 0x47,
  InEarSensitivity: 0x48,
  GameConfig: 0x4a,
  RequestData: 0xfe,
} as const;

export const CMD_NAMES: Record<number, string> = {
  [Cmd.ResetDefault]: "ResetDefault",
  [Cmd.ClearPairing]: "ClearPairing",
  [Cmd.FactoryReset]: "FactoryReset",
  [Cmd.MusicControl]: "MusicControl",
  [Cmd.LightFlash]: "LightFlash",
  [Cmd.InEarDetection]: "InEarDetection",
  [Cmd.NoiseValue]: "NoiseValue",
  [Cmd.Volume]: "Volume",
  [Cmd.LowLatency]: "LowLatency",
  [Cmd.Monitoring]: "Monitoring",
  [Cmd.NoiseCancelMode]: "NoiseCancelMode",
  [Cmd.TestMode]: "TestMode",
  [Cmd.SleepMode]: "SleepMode",
  [Cmd.EarTipFit]: "EarTipFit",
  [Cmd.LedMode]: "LedMode",
  [Cmd.PowerManager]: "PowerManager",
  [Cmd.SoundBalance]: "SoundBalance",
  [Cmd.AncSetting]: "AncSetting",
  [Cmd.RenameDevice]: "RenameDevice",
  [Cmd.VoiceLanguage]: "VoiceLanguage",
  [Cmd.ToneVolume]: "ToneVolume",
  [Cmd.TakePhoto]: "TakePhoto",
  [Cmd.Standby]: "Standby",
  [Cmd.EqParamsV1]: "EqParamsV1",
  [Cmd.EqParamsV2]: "EqParamsV2",
  [Cmd.Ldac]: "Ldac",
  [Cmd.AdaptiveEq]: "AdaptiveEq",
  [Cmd.AncResult]: "AncResult",
  [Cmd.AncWear]: "AncWear",
  [Cmd.KeyFunction]: "KeyFunction",
  [Cmd.WearingDetection]: "WearingDetection",
  [Cmd.SpatialAudio]: "SpatialAudio",
  [Cmd.MusicMode]: "MusicMode",
  [Cmd.Battery]: "Battery",
  [Cmd.Version]: "Version",
  [Cmd.EnvAdaptation]: "EnvAdaptation",
  [Cmd.TwsEnable]: "TwsEnable",
  [Cmd.LedSwitch]: "LedSwitch",
  [Cmd.LedEffect]: "LedEffect",
  [Cmd.PlayMode]: "PlayMode",
  [Cmd.FocusMode]: "FocusMode",
  [Cmd.MusicStatus]: "MusicStatus",
  [Cmd.MusicInfo]: "MusicInfo",
  [Cmd.TonePlay]: "TonePlay",
  [Cmd.SyncTime]: "SyncTime",
  [Cmd.Alarm]: "Alarm",
  [Cmd.Ai]: "Ai",
  [Cmd.MaxEqCount]: "MaxEqCount",
  [Cmd.CustomEqTest]: "CustomEqTest",
  [Cmd.EqLeft]: "EqLeft",
  [Cmd.EqRight]: "EqRight",
  [Cmd.InEarSensitivity]: "InEarSensitivity",
  [Cmd.GameConfig]: "GameConfig",
  [Cmd.RequestData]: "RequestData",
};

export const DESTRUCTIVE_CMDS = new Set<number>([
  Cmd.ResetDefault,
  Cmd.ClearPairing,
  Cmd.FactoryReset,
]);

export type EnableState = "on" | "off";

export function enableByte(state: EnableState): number {
  return state === "on" ? 0x01 : 0x02;
}

export function parseEnable(value: number): boolean {
  return value === 0x01;
}

/** Simple 0x0C noise-cancel modes from the public protocol table. */
export const NoiseMode = {
  Off: 0x00,
  Anc: 0x01,
  Outdoor: 0x02,
  Transparency: 0x03,
} as const;

export type NoiseModeId = (typeof NoiseMode)[keyof typeof NoiseMode];

export const MusicAction = {
  Play: 0x01,
  Pause: 0x02,
  Previous: 0x03,
  Next: 0x04,
} as const;

export const KeyId = {
  MusicLeftSingle: 0x01,
  MusicRightSingle: 0x02,
  MusicLeftDouble: 0x03,
  MusicRightDouble: 0x04,
  MusicLeftTriple: 0x05,
  MusicRightTriple: 0x06,
  MusicLeftQuad: 0x07,
  MusicRightQuad: 0x08,
  MusicLeftHold: 0x09,
  MusicRightHold: 0x0a,
  CallLeftSingle: 0x15,
  CallRightSingle: 0x16,
  CallLeftDouble: 0x17,
  CallRightDouble: 0x18,
  CallLeftTriple: 0x19,
  CallRightTriple: 0x1a,
  CallLeftQuad: 0x1b,
  CallRightQuad: 0x1c,
  CallLeftHold: 0x1d,
  CallRightHold: 0x1e,
} as const;

export const FunId = {
  None: 0x00,
  PlayPause: 0x01,
  Previous: 0x02,
  Next: 0x03,
  VoiceAssistant: 0x04,
  VolumeUp: 0x05,
  VolumeDown: 0x06,
  GameMode: 0x07,
  AnswerCall: 0x08,
  RejectCall: 0x09,
  HoldCall: 0x0a,
  Redial: 0x0b,
} as const;

export const KEY_LABELS: Record<number, string> = {
  [KeyId.MusicLeftSingle]: "Left single tap",
  [KeyId.MusicRightSingle]: "Right single tap",
  [KeyId.MusicLeftDouble]: "Left double tap",
  [KeyId.MusicRightDouble]: "Right double tap",
  [KeyId.MusicLeftTriple]: "Left triple tap",
  [KeyId.MusicRightTriple]: "Right triple tap",
  [KeyId.MusicLeftQuad]: "Left quad tap",
  [KeyId.MusicRightQuad]: "Right quad tap",
  [KeyId.MusicLeftHold]: "Left hold",
  [KeyId.MusicRightHold]: "Right hold",
  [KeyId.CallLeftSingle]: "Call · left single",
  [KeyId.CallRightSingle]: "Call · right single",
  [KeyId.CallLeftDouble]: "Call · left double",
  [KeyId.CallRightDouble]: "Call · right double",
  [KeyId.CallLeftTriple]: "Call · left triple",
  [KeyId.CallRightTriple]: "Call · right triple",
  [KeyId.CallLeftQuad]: "Call · left quad",
  [KeyId.CallRightQuad]: "Call · right quad",
  [KeyId.CallLeftHold]: "Call · left hold",
  [KeyId.CallRightHold]: "Call · right hold",
};

export const FUN_LABELS: Record<number, string> = {
  [FunId.None]: "None",
  [FunId.PlayPause]: "Play / Pause",
  [FunId.Previous]: "Previous",
  [FunId.Next]: "Next",
  [FunId.VoiceAssistant]: "Voice assistant",
  [FunId.VolumeUp]: "Volume up",
  [FunId.VolumeDown]: "Volume down",
  [FunId.GameMode]: "Game mode",
  [FunId.AnswerCall]: "Answer call",
  [FunId.RejectCall]: "Reject call",
  [FunId.HoldCall]: "Hold call",
  [FunId.Redial]: "Redial",
};

export type BatteryCell = {
  level: number;
  charging: boolean;
};

export type BatteryState = {
  left: BatteryCell;
  right: BatteryCell;
  case: BatteryCell;
};

export type FirmwareVersion = {
  left: string;
  right?: string;
};

export type CommandBlock = {
  cmd: number;
  params: Uint8Array;
};

export type Packet = {
  blocks: CommandBlock[];
  raw: Uint8Array;
};

export type DecodeError = {
  kind: "too-short" | "bad-sof" | "length-mismatch" | "truncated-block" | "oversize";
  message: string;
};

export type DecodeResult =
  | { ok: true; packet: Packet }
  | { ok: false; error: DecodeError };

export type Advertisement = {
  companyId: number;
  vendorId: number;
  colorIndex: number;
  battery: BatteryState;
  controlMac: string;
  otherMac: string;
  rawLength: number;
};

export type AncScene = {
  mode: number;
  subScene: number;
  noiseValue: number;
};

export type EqBand = {
  freqHz: number;
  gainDb: number;
  q: number;
  bandType?: number;
};

export type EqPreset = {
  index: number;
  masterGainDb: number;
  bands: EqBand[];
};

export type KeyBinding = {
  keyId: number;
  funId: number;
};

export type WearSettings = {
  enabled: boolean;
  musicIndex: number;
  ancIndex: number;
  toneEnable?: boolean;
};
