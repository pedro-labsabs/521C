import { encodeCommand } from "./packet";
import {
  Cmd,
  enableByte,
  type AncScene,
  type EnableState,
  type EqPreset,
  type KeyBinding,
  type WearSettings,
} from "./types";

function i16le(n: number): [number, number] {
  const v = n & 0xffff;
  return [v & 0xff, (v >> 8) & 0xff];
}

function s16leFromDb(db: number): [number, number] {
  const clamped = Math.max(-12.7, Math.min(12.7, db));
  const raw = Math.round(clamped * 100);
  return i16le(raw < 0 ? raw + 0x10000 : raw);
}

function u16leFromHz(hz: number): [number, number] {
  return i16le(Math.max(0, Math.min(65535, Math.round(hz))));
}

function u16leFromQ(q: number): [number, number] {
  return i16le(Math.max(0, Math.min(65535, Math.round(q * 100))));
}

export const request = {
  data: (cmdId: number) => encodeCommand(Cmd.RequestData, [cmdId & 0xff]),
  battery: () => encodeCommand(Cmd.RequestData, [Cmd.Battery]),
  version: () => encodeCommand(Cmd.RequestData, [Cmd.Version]),
  noiseMode: () => encodeCommand(Cmd.RequestData, [Cmd.NoiseCancelMode]),
  ancSetting: () => encodeCommand(Cmd.RequestData, [Cmd.AncSetting]),
  lowLatency: () => encodeCommand(Cmd.RequestData, [Cmd.LowLatency]),
  inEar: () => encodeCommand(Cmd.RequestData, [Cmd.InEarDetection]),
  sleep: () => encodeCommand(Cmd.RequestData, [Cmd.SleepMode]),
  spatial: () => encodeCommand(Cmd.RequestData, [Cmd.SpatialAudio]),
  eqV2: () => encodeCommand(Cmd.RequestData, [Cmd.EqParamsV2]),
  keyFunction: () => encodeCommand(Cmd.RequestData, [Cmd.KeyFunction]),
  wear: () => encodeCommand(Cmd.RequestData, [Cmd.WearingDetection]),
  ldac: () => encodeCommand(Cmd.RequestData, [Cmd.Ldac]),
  envAdaptation: () => encodeCommand(Cmd.RequestData, [Cmd.EnvAdaptation]),
};

export const set = {
  noiseMode: (mode: number) => encodeCommand(Cmd.NoiseCancelMode, [mode & 0xff]),
  ancSetting: (scene: AncScene) =>
    encodeCommand(Cmd.AncSetting, [scene.mode & 0xff, scene.subScene & 0xff, scene.noiseValue & 0xff]),
  lowLatency: (state: EnableState) => encodeCommand(Cmd.LowLatency, [enableByte(state)]),
  inEar: (state: EnableState) => encodeCommand(Cmd.InEarDetection, [enableByte(state)]),
  sleep: (state: EnableState) => encodeCommand(Cmd.SleepMode, [enableByte(state)]),
  spatial: (state: EnableState) => encodeCommand(Cmd.SpatialAudio, [enableByte(state)]),
  ldac: (state: EnableState) => encodeCommand(Cmd.Ldac, [enableByte(state)]),
  envAdaptation: (state: EnableState) => encodeCommand(Cmd.EnvAdaptation, [enableByte(state)]),
  lightFlash: (on: boolean) => encodeCommand(Cmd.LightFlash, [on ? 0x01 : 0x00]),
  tonePlay: (toneId: number) => encodeCommand(Cmd.TonePlay, [toneId & 0xff]),
  music: (action: number) => encodeCommand(Cmd.MusicControl, [action & 0xff]),
  volume: (left: number, right: number) =>
    encodeCommand(Cmd.Volume, [left & 0xff, right & 0xff, 0x00]),
  soundBalance: (value: number) => encodeCommand(Cmd.SoundBalance, [value & 0xff]),
  noiseValue: (value: number) => encodeCommand(Cmd.NoiseValue, [value & 0xff]),
  wear: (settings: WearSettings) => {
    const params = [
      settings.enabled ? 0x01 : 0x02,
      settings.musicIndex & 0xff,
      settings.ancIndex & 0xff,
    ];
    if (settings.toneEnable !== undefined) {
      params.push(settings.toneEnable ? 0x01 : 0x02);
    }
    return encodeCommand(Cmd.WearingDetection, params);
  },
  eqV2: (preset: EqPreset) => {
    const params: number[] = [preset.index & 0xff, ...s16leFromDb(preset.masterGainDb)];
    for (const band of preset.bands) {
      params.push(
        ...u16leFromHz(band.freqHz),
        ...s16leFromDb(band.gainDb),
        ...u16leFromQ(band.q),
        (band.bandType ?? 0) & 0xff,
      );
    }
    return encodeCommand(Cmd.EqParamsV2, params);
  },
  rename: (name: string) => {
    const bytes = new TextEncoder().encode(name);
    return encodeCommand(Cmd.RenameDevice, bytes);
  },
};

export function encodeKeyFunctionDirect(bindings: KeyBinding[]): Uint8Array {
  const out = new Uint8Array(bindings.length * 2);
  bindings.forEach((b, i) => {
    out[i * 2] = b.keyId & 0xff;
    out[i * 2 + 1] = b.funId & 0xff;
  });
  return out;
}

export function parseKeyFunctionBytes(bytes: ArrayLike<number>): KeyBinding[] {
  const d = bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes);
  const out: KeyBinding[] = [];
  for (let i = 0; i + 1 < d.length; i += 2) {
    out.push({ keyId: d[i]!, funId: d[i + 1]! });
  }
  return out;
}

function s16leToDb(lo: number, hi: number): number {
  let v = lo | (hi << 8);
  if (v & 0x8000) v -= 0x10000;
  return v / 100;
}

export function parseEqV2(params: ArrayLike<number>): EqPreset | null {
  const d = params instanceof Uint8Array ? params : Uint8Array.from(params);
  if (d.length < 3) return null;
  const index = d[0]!;
  const masterGainDb = s16leToDb(d[1]!, d[2]!);
  const bands: EqPreset["bands"] = [];
  let i = 3;
  while (i + 7 <= d.length) {
    const freqHz = d[i]! | (d[i + 1]! << 8);
    const gainDb = s16leToDb(d[i + 2]!, d[i + 3]!);
    const q = (d[i + 4]! | (d[i + 5]! << 8)) / 100;
    const bandType = d[i + 6]!;
    bands.push({ freqHz, gainDb, q, bandType });
    i += 7;
  }
  return { index, masterGainDb, bands };
}

export function parseAncScene(params: ArrayLike<number>): AncScene | null {
  const d = params instanceof Uint8Array ? params : Uint8Array.from(params);
  if (d.length < 3) return null;
  return { mode: d[0]!, subScene: d[1]!, noiseValue: d[2]! };
}

export function parseWear(params: ArrayLike<number>): WearSettings | null {
  const d = params instanceof Uint8Array ? params : Uint8Array.from(params);
  if (d.length < 3) return null;
  return {
    enabled: d[0] === 0x01,
    musicIndex: d[1]!,
    ancIndex: d[2]!,
    toneEnable: d.length >= 4 ? d[3] === 0x01 : undefined,
  };
}
