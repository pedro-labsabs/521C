import { QCY_COMPANY_ID } from "./uuids";
import type { Advertisement, BatteryCell, BatteryState } from "./types";

function parseCell(byte: number | undefined): BatteryCell {
  const b = byte ?? 0;
  return {
    level: Math.min(100, b & 0x7f),
    charging: (b & 0x80) !== 0,
  };
}

export function encodeCell(cell: BatteryCell): number {
  const level = Math.max(0, Math.min(127, Math.round(cell.level)));
  return (cell.charging ? 0x80 : 0) | level;
}

function formatMac(bytes: number[]): string {
  return bytes.map((b) => b.toString(16).padStart(2, "0")).join(":").toUpperCase();
}

/**
 * Manufacturer data CompanyID 0x521c.
 * Control MAC scrambled order: [12]:[11]:[13]:[16]:[15]:[14]
 * Other MAC scrambled order:   [19]:[18]:[20]:[23]:[22]:[21]
 */
export function parseManufacturerData(
  companyId: number,
  data: ArrayLike<number>,
): Advertisement | null {
  if (companyId !== QCY_COMPANY_ID) return null;
  const d = data instanceof Uint8Array ? data : Uint8Array.from(data);
  if (d.length < 8) return null;

  const vendorId = ((d[0] ?? 0) << 8) | (d[1] ?? 0);
  const colorIndex = ((d[3] ?? 0) & 0x18) >> 1;
  const battery: BatteryState = {
    left: parseCell(d[5]),
    right: parseCell(d[6]),
    case: parseCell(d[7]),
  };

  let controlMac = "00:00:00:00:00:00";
  let otherMac = "00:00:00:00:00:00";
  if (d.length >= 17) {
    controlMac = formatMac([d[12] ?? 0, d[11] ?? 0, d[13] ?? 0, d[16] ?? 0, d[15] ?? 0, d[14] ?? 0]);
  }
  if (d.length >= 24) {
    otherMac = formatMac([d[19] ?? 0, d[18] ?? 0, d[20] ?? 0, d[23] ?? 0, d[22] ?? 0, d[21] ?? 0]);
  }
  if (otherMac === "00:00:00:00:00:00") {
    otherMac = controlMac;
  }

  return {
    companyId,
    vendorId,
    colorIndex,
    battery,
    controlMac,
    otherMac,
    rawLength: d.length,
  };
}

export function encodeManufacturerData(input: {
  vendorId: number;
  colorIndex?: number;
  battery: BatteryState;
  controlMac: string;
  otherMac?: string;
}): Uint8Array {
  const d = new Uint8Array(24);
  d[0] = (input.vendorId >> 8) & 0xff;
  d[1] = input.vendorId & 0xff;
  const color = input.colorIndex ?? 0;
  d[3] = (color << 1) & 0x18;
  d[5] = encodeCell(input.battery.left);
  d[6] = encodeCell(input.battery.right);
  d[7] = encodeCell(input.battery.case);

  const writeScrambled = (mac: string, indices: number[]) => {
    const parts = mac.split(":").map((p) => parseInt(p, 16) || 0);
    // inverse of [12,11,13,16,15,14] display order
    const display = parts.length === 6 ? parts : [0, 0, 0, 0, 0, 0];
    d[indices[0]!] = display[0]!;
    d[indices[1]!] = display[1]!;
    d[indices[2]!] = display[2]!;
    d[indices[3]!] = display[3]!;
    d[indices[4]!] = display[4]!;
    d[indices[5]!] = display[5]!;
  };

  // control display [12]:[11]:[13]:[16]:[15]:[14] so store:
  // display[0]->12, [1]->11, [2]->13, [3]->16, [4]->15, [5]->14
  writeScrambled(input.controlMac, [12, 11, 13, 16, 15, 14]);
  writeScrambled(input.otherMac ?? input.controlMac, [19, 18, 20, 23, 22, 21]);
  return d;
}

export function parseBatteryBytes(bytes: ArrayLike<number>): BatteryState | null {
  const d = bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes);
  if (d.length < 3) return null;
  return {
    left: parseCell(d[0]),
    right: parseCell(d[1]),
    case: parseCell(d[2]),
  };
}

export function encodeBatteryBytes(battery: BatteryState): Uint8Array {
  return Uint8Array.from([
    encodeCell(battery.left),
    encodeCell(battery.right),
    encodeCell(battery.case),
  ]);
}

export function parseFirmwareBytes(bytes: ArrayLike<number>): { left: string; right?: string } | null {
  const d = bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes);
  if (d.length >= 6) {
    return {
      left: `${d[0]}.${d[1]}.${d[2]}`,
      right: `${d[3]}.${d[4]}.${d[5]}`,
    };
  }
  if (d.length >= 3) {
    return { left: `${d[0]}.${d[1]}.${d[2]}` };
  }
  return null;
}
