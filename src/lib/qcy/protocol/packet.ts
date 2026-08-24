import { CMD_NAMES, SOF, type CommandBlock, type DecodeResult, type Packet } from "./types";

const MAX_PACKET = 512;

export function encodeBlocks(blocks: CommandBlock[]): Uint8Array {
  const body: number[] = [];
  for (const block of blocks) {
    if (block.cmd < 0 || block.cmd > 0xff) {
      throw new Error(`opcode out of range: ${block.cmd}`);
    }
    if (block.params.length > 255) {
      throw new Error(`param length exceeds 255 for cmd 0x${block.cmd.toString(16)}`);
    }
    body.push(block.cmd, block.params.length, ...block.params);
  }
  if (body.length > 255) {
    throw new Error("packet body exceeds 255 bytes");
  }
  return Uint8Array.from([SOF, body.length, ...body]);
}

export function encodeCommand(cmd: number, params: ArrayLike<number> = []): Uint8Array {
  return encodeBlocks([{ cmd, params: Uint8Array.from(params) }]);
}

export function decodePacket(bytes: ArrayLike<number>): DecodeResult {
  const data = bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes);
  if (data.length < 2) {
    return { ok: false, error: { kind: "too-short", message: "packet shorter than header" } };
  }
  if (data.length > MAX_PACKET) {
    return { ok: false, error: { kind: "oversize", message: `packet ${data.length} > ${MAX_PACKET}` } };
  }
  if (data[0] !== SOF) {
    return {
      ok: false,
      error: { kind: "bad-sof", message: `expected 0xFF, got 0x${data[0]!.toString(16)}` },
    };
  }
  const bodyLen = data[1]!;
  if (data.length !== bodyLen + 2) {
    return {
      ok: false,
      error: {
        kind: "length-mismatch",
        message: `body_len=${bodyLen} but buffer is ${data.length} bytes`,
      },
    };
  }
  const blocks: CommandBlock[] = [];
  let i = 2;
  while (i < data.length) {
    if (i + 2 > data.length) {
      return {
        ok: false,
        error: { kind: "truncated-block", message: "truncated command header" },
      };
    }
    const cmd = data[i]!;
    const paramLen = data[i + 1]!;
    if (i + 2 + paramLen > data.length) {
      return {
        ok: false,
        error: {
          kind: "truncated-block",
          message: `cmd 0x${cmd.toString(16)} claims ${paramLen} param bytes past end`,
        },
      };
    }
    blocks.push({ cmd, params: data.slice(i + 2, i + 2 + paramLen) });
    i += 2 + paramLen;
  }
  return { ok: true, packet: { blocks, raw: data } };
}

export function cmdName(cmd: number): string {
  return CMD_NAMES[cmd] ?? `Unknown(0x${cmd.toString(16).padStart(2, "0")})`;
}

export function toHex(bytes: ArrayLike<number>, sep = " "): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join(sep);
}

export function fromHex(hex: string): Uint8Array {
  const clean = hex.replace(/[^0-9a-fA-F]/g, "");
  if (clean.length % 2 !== 0) {
    throw new Error("odd hex length");
  }
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

export function describePacket(packet: Packet): string {
  return packet.blocks
    .map((b) => `${cmdName(b.cmd)}[${toHex(b.params)}]`)
    .join(" | ");
}
