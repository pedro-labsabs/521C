import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  decodePacket,
  encodeBlocks,
  encodeCommand,
  fromHex,
  toHex,
} from "./packet";
import {
  parseBatteryBytes,
  parseFirmwareBytes,
  parseManufacturerData,
} from "./advertisement";
import { request, set } from "./commands";
import { DEVICE_EQ_PRESETS } from "../eq-presets";
import type { CommandBlock } from "./types";

/**
 * Shared conformance vectors. The same JSON corpus drives the Rust
 * `qcy-protocol` tests, so a cross-language semantic divergence covered here
 * fails at least one repository gate. See conformance/README.md.
 */
const corpus = JSON.parse(
  readFileSync(
    new URL("../../../../conformance/protocol_vectors.json", import.meta.url),
    "utf8",
  ),
) as Corpus;

type BlockVector = { cmd: number; paramsHex: string };
type BatteryCellVector = { level: number; charging: boolean };
type BatteryVector = {
  left: BatteryCellVector;
  right: BatteryCellVector;
  case: BatteryCellVector;
};
type DecodeExpect =
  | { ok: true; blocks: BlockVector[] }
  | { ok: false; error: string };
type DecodeVector = { name: string; hex: string; expect: DecodeExpect };
type EncodeVector = { name: string; blocks: BlockVector[]; expectHex: string };
type AdvExpect = {
  vendorId: number;
  battery: BatteryVector;
  controlMac: string;
  otherMac: string;
};
type AdvVector = {
  name: string;
  companyId: number;
  dataHex: string;
  expect: AdvExpect | null;
};
type BatteryParseVector = { name: string; hex: string; expect: BatteryVector | null };
type FirmwareVector = {
  name: string;
  hex: string;
  expect: { left: string; right?: string } | null;
};
type Corpus = {
  version: number;
  decode: DecodeVector[];
  encode: EncodeVector[];
  advertisement: AdvVector[];
  battery: BatteryParseVector[];
  firmware: { vectors: FirmwareVector[] };
};

function blocksFromVector(blocks: BlockVector[]): CommandBlock[] {
  return blocks.map((b) => ({ cmd: b.cmd, params: fromHex(b.paramsHex) }));
}

describe("shared conformance vectors · frame decode", () => {
  for (const v of corpus.decode) {
    it(`decode: ${v.name}`, () => {
      const result = decodePacket(fromHex(v.hex));
      if (v.expect.ok) {
        expect(result.ok, v.name).toBe(true);
        if (!result.ok) return;
        const expected = v.expect;
        expect(result.packet.blocks.length).toBe(expected.blocks.length);
        result.packet.blocks.forEach((block, i) => {
          const want = expected.blocks[i]!;
          expect(block.cmd).toBe(want.cmd);
          expect(toHex(block.params, "")).toBe(want.paramsHex);
        });
      } else {
        expect(result.ok, v.name).toBe(false);
        if (!result.ok) expect(result.error.kind).toBe(v.expect.error);
      }
    });
  }
});

describe("shared conformance vectors · frame encode", () => {
  for (const v of corpus.encode) {
    it(`encode: ${v.name}`, () => {
      const bytes = encodeBlocks(blocksFromVector(v.blocks));
      expect(toHex(bytes, "")).toBe(v.expectHex);
    });
  }
});

describe("shared conformance vectors · advertisement", () => {
  for (const v of corpus.advertisement) {
    it(`advertisement: ${v.name}`, () => {
      const adv = parseManufacturerData(v.companyId, fromHex(v.dataHex));
      if (v.expect === null) {
        expect(adv).toBeNull();
        return;
      }
      expect(adv).not.toBeNull();
      if (!adv) return;
      expect(adv.vendorId).toBe(v.expect.vendorId);
      expect(adv.battery.left).toEqual(v.expect.battery.left);
      expect(adv.battery.right).toEqual(v.expect.battery.right);
      expect(adv.battery.case).toEqual(v.expect.battery.case);
      expect(adv.controlMac).toBe(v.expect.controlMac);
      expect(adv.otherMac).toBe(v.expect.otherMac);
    });
  }
});

describe("shared conformance vectors · battery bytes", () => {
  for (const v of corpus.battery) {
    it(`battery: ${v.name}`, () => {
      const parsed = parseBatteryBytes(fromHex(v.hex));
      if (v.expect === null) {
        expect(parsed).toBeNull();
        return;
      }
      expect(parsed).toEqual(v.expect);
    });
  }
});

describe("typescript-only vectors · firmware bytes", () => {
  for (const v of corpus.firmware.vectors) {
    it(`firmware: ${v.name}`, () => {
      const parsed = parseFirmwareBytes(fromHex(v.hex));
      if (v.expect === null) {
        expect(parsed).toBeNull();
        return;
      }
      expect(parsed).toEqual(v.expect);
    });
  }
});

describe("framing boundaries (programmatic)", () => {
  it("rejects a buffer larger than maxPacket as oversize", () => {
    const big = new Uint8Array(600);
    big[0] = 0xff;
    const result = decodePacket(big);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error.kind).toBe("oversize");
  });

  it("rejects a block with more than 255 param bytes", () => {
    expect(() => encodeCommand(0x22, new Uint8Array(256))).toThrow();
  });

  it("rejects a packet body exceeding 255 bytes", () => {
    const blocks: CommandBlock[] = Array.from({ length: 100 }, () => ({
      cmd: 0x09,
      params: Uint8Array.from([0x01]),
    }));
    expect(() => encodeBlocks(blocks)).toThrow();
  });
});

describe("round-trip of currently enabled write commands", () => {
  const cases: Array<{ name: string; bytes: Uint8Array; cmd: number }> = [
    { name: "noiseMode", bytes: set.noiseMode(0x01), cmd: 0x0c },
    {
      name: "ancSetting",
      bytes: set.ancSetting({ mode: 0x02, subScene: 0x02, noiseValue: 80 }),
      cmd: 0x17,
    },
    { name: "lowLatency", bytes: set.lowLatency("on"), cmd: 0x09 },
    { name: "inEar", bytes: set.inEar("on"), cmd: 0x06 },
    { name: "sleep", bytes: set.sleep("on"), cmd: 0x10 },
    { name: "spatial", bytes: set.spatial("on"), cmd: 0x2d },
    { name: "ldac", bytes: set.ldac("on"), cmd: 0x23 },
    { name: "envAdaptation", bytes: set.envAdaptation("on"), cmd: 0x32 },
    { name: "lightFlash", bytes: set.lightFlash(true), cmd: 0x05 },
    { name: "tonePlay", bytes: set.tonePlay(1), cmd: 0x3d },
    { name: "music", bytes: set.music(0x01), cmd: 0x04 },
    { name: "volume", bytes: set.volume(10, 20), cmd: 0x08 },
    { name: "soundBalance", bytes: set.soundBalance(50), cmd: 0x16 },
    { name: "noiseValue", bytes: set.noiseValue(80), cmd: 0x07 },
    {
      name: "wear",
      bytes: set.wear({ enabled: true, musicIndex: 1, ancIndex: 0, toneEnable: true }),
      cmd: 0x2c,
    },
    { name: "eqV2", bytes: set.eqV2(DEVICE_EQ_PRESETS[0]!.preset), cmd: 0x22 },
    { name: "rename", bytes: set.rename("521C"), cmd: 0x18 },
    { name: "request.battery", bytes: request.battery(), cmd: 0xfe },
    { name: "request.version", bytes: request.version(), cmd: 0xfe },
  ];

  for (const c of cases) {
    it(`round-trips ${c.name}`, () => {
      const decoded = decodePacket(c.bytes);
      expect(decoded.ok, c.name).toBe(true);
      if (!decoded.ok) return;
      expect(decoded.packet.blocks[0]!.cmd).toBe(c.cmd);
      // Re-encoding the decoded block must reproduce the original bytes.
      const reencoded = encodeBlocks(decoded.packet.blocks);
      expect(toHex(reencoded, "")).toBe(toHex(c.bytes, ""));
    });
  }
});
