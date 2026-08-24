import type { EqBand, EqPreset } from "./protocol/types";

const FREQS = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

function bands(gains: number[]): EqBand[] {
  return FREQS.map((freqHz, i) => ({
    freqHz,
    gainDb: gains[i] ?? 0,
    q: 1.0,
    bandType: 0,
  }));
}

export type NamedEq = {
  id: string;
  name: string;
  kind: "device" | "system";
  official: boolean;
  preset: EqPreset;
};

function named(id: string, name: string, gains: number[], official = false): NamedEq {
  return {
    id,
    name,
    kind: "device",
    official,
    preset: { index: 0, masterGainDb: 0, bands: bands(gains) },
  };
}

/** Community band tables written through the documented 0x22 parametric path.
 *  Names follow common QCY-app style labels; they are not claimed as dumped official dumps.
 */
export const DEVICE_EQ_PRESETS: NamedEq[] = [
  named("flat", "Flat", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0], true),
  named("bass", "Bass boost", [6, 5, 3, 1, 0, 0, 0, 0, 0, 0]),
  named("treble", "Treble boost", [0, 0, 0, 0, 0, 1, 2, 4, 5, 4]),
  named("voice", "Voice", [-2, -1, 0, 1, 3, 4, 3, 1, 0, -1]),
  named("classical", "Classical", [3, 2, 0, 0, 0, 0, 1, 2, 3, 3]),
  named("rock", "Rock", [4, 3, 1, 0, -1, 0, 2, 3, 3, 2]),
  named("pop", "Pop", [-1, 0, 1, 2, 3, 2, 1, 1, 2, 1]),
  named("jazz", "Jazz", [2, 1, 0, 1, 2, 1, 0, 1, 2, 2]),
  named("gaming", "Gaming", [3, 2, 1, 0, 0, 1, 2, 3, 2, 1]),
];

export function eqGains(preset: EqPreset): number[] {
  return FREQS.map((f) => preset.bands.find((b) => b.freqHz === f)?.gainDb ?? 0);
}

export function presetFromGains(gains: number[], masterGainDb = 0): EqPreset {
  return { index: 0, masterGainDb, bands: bands(gains) };
}

export const EQ_FREQS = FREQS;
