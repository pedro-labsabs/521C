import assert from "node:assert/strict";
import { test } from "node:test";

const SOF = 0xff;

function encodeBlocks(blocks) {
  const body = [];
  for (const block of blocks) {
    body.push(block.cmd, block.params.length, ...block.params);
  }
  return Uint8Array.from([SOF, body.length, ...body]);
}

function decodePacket(data) {
  if (data.length < 2) return { ok: false, kind: "too-short" };
  if (data[0] !== SOF) return { ok: false, kind: "bad-sof" };
  const bodyLen = data[1];
  if (data.length !== bodyLen + 2) return { ok: false, kind: "length-mismatch" };
  const blocks = [];
  let i = 2;
  while (i < data.length) {
    if (i + 2 > data.length) return { ok: false, kind: "truncated-block" };
    const cmd = data[i];
    const paramLen = data[i + 1];
    if (i + 2 + paramLen > data.length) return { ok: false, kind: "truncated-block" };
    blocks.push({ cmd, params: data.slice(i + 2, i + 2 + paramLen) });
    i += 2 + paramLen;
  }
  return { ok: true, blocks };
}

function parseCell(b) {
  return { level: b & 0x7f, charging: (b & 0x80) !== 0 };
}

function parseManufacturerData(companyId, d) {
  if (companyId !== 0x521c || d.length < 8) return null;
  const vendorId = (d[0] << 8) | d[1];
  const battery = {
    left: parseCell(d[5]),
    right: parseCell(d[6]),
    case: parseCell(d[7]),
  };
  let controlMac = "00:00:00:00:00:00";
  if (d.length >= 17) {
    const fmt = (arr) => arr.map((x) => x.toString(16).padStart(2, "0")).join(":").toUpperCase();
    controlMac = fmt([d[12], d[11], d[13], d[16], d[15], d[14]]);
  }
  return { vendorId, battery, controlMac };
}

test("encode battery request 0xFE 0x2F", () => {
  const pkt = encodeBlocks([{ cmd: 0xfe, params: [0x2f] }]);
  assert.deepEqual([...pkt], [0xff, 0x03, 0xfe, 0x01, 0x2f]);
});

test("encode ANC indoor level 2", () => {
  const pkt = encodeBlocks([{ cmd: 0x17, params: [0x02, 0x02, 80] }]);
  assert.equal(pkt[0], 0xff);
  assert.equal(pkt[1], 5);
  assert.equal(pkt[2], 0x17);
  assert.equal(pkt[3], 3);
  assert.equal(pkt[4], 0x02);
  assert.equal(pkt[5], 0x02);
  assert.equal(pkt[6], 80);
});

test("decode multi-block packet", () => {
  const pkt = encodeBlocks([
    { cmd: 0x2f, params: [0x52, 0x50, 0x5e] },
    { cmd: 0x09, params: [0x01] },
  ]);
  const d = decodePacket(pkt);
  assert.equal(d.ok, true);
  assert.equal(d.blocks.length, 2);
  assert.equal(d.blocks[0].cmd, 0x2f);
  assert.deepEqual([...d.blocks[0].params], [0x52, 0x50, 0x5e]);
  assert.equal(d.blocks[1].cmd, 0x09);
});

test("reject bad SOF", () => {
  const d = decodePacket(Uint8Array.from([0x00, 0x01, 0x00]));
  assert.equal(d.ok, false);
  assert.equal(d.kind, "bad-sof");
});

test("reject length mismatch", () => {
  const d = decodePacket(Uint8Array.from([0xff, 0x10, 0x01]));
  assert.equal(d.ok, false);
  assert.equal(d.kind, "length-mismatch");
});

test("reject truncated block", () => {
  const d = decodePacket(Uint8Array.from([0xff, 0x03, 0x2f, 0x05, 0x00]));
  assert.equal(d.ok, false);
  assert.equal(d.kind, "truncated-block");
});

test("battery charging flag", () => {
  const cell = parseCell(0x80 | 42);
  assert.equal(cell.level, 42);
  assert.equal(cell.charging, true);
});

test("manufacturer MAC scramble", () => {
  const d = new Uint8Array(24);
  d[0] = 0x12;
  d[1] = 0x34;
  d[5] = 80;
  d[6] = 70;
  d[7] = 90;
  // display AA:BB:CC:DD:EE:FF stored as [12]=AA [11]=BB [13]=CC [16]=DD [15]=EE [14]=FF
  d[12] = 0xaa;
  d[11] = 0xbb;
  d[13] = 0xcc;
  d[16] = 0xdd;
  d[15] = 0xee;
  d[14] = 0xff;
  const adv = parseManufacturerData(0x521c, d);
  assert.equal(adv.vendorId, 0x1234);
  assert.equal(adv.controlMac, "AA:BB:CC:DD:EE:FF");
  assert.equal(adv.battery.left.level, 80);
});

test("ignore non-QCY company id", () => {
  assert.equal(parseManufacturerData(0x004c, new Uint8Array(24)), null);
});

test("low latency enable byte", () => {
  const pkt = encodeBlocks([{ cmd: 0x09, params: [0x01] }]);
  const d = decodePacket(pkt);
  assert.equal(d.blocks[0].params[0], 0x01);
});
