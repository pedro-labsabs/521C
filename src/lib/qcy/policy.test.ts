import { describe, expect, it } from "vitest";
import {
  authorizeDirectWrite,
  authorizeFrameWrite,
  DEFAULT_OPT_IN,
  WriteDeniedError,
  type SessionOptIn,
} from "./policy";
import { GENERIC_QCY_PROFILE, HT08_PROFILE } from "./device/catalog";
import { encodeBlocks, encodeCommand } from "./protocol";
import { CHAR, Cmd } from "./protocol";
import { MockTransport } from "./transport";

const NO_OPT_IN: SessionOptIn = { ...DEFAULT_OPT_IN };
const OPTED_IN: SessionOptIn = { experimental: true };

function denialOf(result: { ok: boolean; denial?: unknown }) {
  return result.ok ? null : (result as { denial: { code: string } }).denial;
}

describe("central write policy · destructive opcodes", () => {
  const destructive = [Cmd.ResetDefault, Cmd.ClearPairing, Cmd.FactoryReset];

  for (const cmd of destructive) {
    it(`rejects destructive 0x${cmd.toString(16)} even as a raw encoded frame`, () => {
      const frame = encodeCommand(cmd, []);
      const result = authorizeFrameWrite(HT08_PROFILE, OPTED_IN, frame);
      expect(result.ok).toBe(false);
      expect(denialOf(result)?.code).toBe("destructive-opcode");
    });
  }

  it("rejects a destructive opcode embedded in a multi-block frame", () => {
    const frame = encodeBlocks([
      { cmd: Cmd.LowLatency, params: Uint8Array.from([0x01]) },
      { cmd: Cmd.FactoryReset, params: Uint8Array.from([]) },
    ]);
    const result = authorizeFrameWrite(HT08_PROFILE, OPTED_IN, frame);
    expect(result.ok).toBe(false);
    expect(denialOf(result)?.code).toBe("destructive-opcode");
  });
});

describe("central write policy · unknown/generic devices are read-only", () => {
  it("denies a supported HT08 opcode on the generic profile", () => {
    const frame = encodeCommand(Cmd.LowLatency, [0x01]);
    const result = authorizeFrameWrite(GENERIC_QCY_PROFILE, OPTED_IN, frame);
    expect(result.ok).toBe(false);
    expect(denialOf(result)?.code).toBe("device-read-only");
  });

  it("denies direct characteristic writes on the generic profile", () => {
    const result = authorizeDirectWrite(
      GENERIC_QCY_PROFILE,
      OPTED_IN,
      CHAR.keyFunctionV2,
      Uint8Array.from([0x01, 0x01]),
    );
    expect(result.ok).toBe(false);
    expect(denialOf(result)?.code).toBe("device-read-only");
  });

  it("still allows RequestData read-back on the generic profile", () => {
    const frame = encodeCommand(Cmd.RequestData, [Cmd.Battery]);
    const result = authorizeFrameWrite(GENERIC_QCY_PROFILE, NO_OPT_IN, frame);
    expect(result.ok).toBe(true);
  });
});

describe("central write policy · HT08 supported writes", () => {
  const supported: Array<[string, Uint8Array]> = [
    ["ancSetting", encodeCommand(Cmd.AncSetting, [0x02, 0x02, 80])],
    ["lowLatency", encodeCommand(Cmd.LowLatency, [0x01])],
    ["inEar", encodeCommand(Cmd.InEarDetection, [0x01])],
    ["sleep", encodeCommand(Cmd.SleepMode, [0x01])],
    ["soundBalance", encodeCommand(Cmd.SoundBalance, [50])],
    ["eqV2", encodeCommand(Cmd.EqParamsV2, [0, 0, 0])],
    ["wearing", encodeCommand(Cmd.WearingDetection, [0x01, 0x01, 0x00])],
  ];

  for (const [name, frame] of supported) {
    it(`allows supported write: ${name}`, () => {
      expect(authorizeFrameWrite(HT08_PROFILE, NO_OPT_IN, frame).ok).toBe(true);
    });
  }

  it("denies an opcode that is not writable for HT08 (undocumented MusicControl)", () => {
    const frame = encodeCommand(Cmd.MusicControl, [0x01]);
    const result = authorizeFrameWrite(HT08_PROFILE, OPTED_IN, frame);
    expect(result.ok).toBe(false);
    expect(denialOf(result)?.code).toBe("opcode-not-writable");
  });
});

describe("central write policy · experimental opt-in", () => {
  it("denies enabling an experimental opcode without opt-in", () => {
    const frame = encodeCommand(Cmd.SpatialAudio, [0x01]);
    const result = authorizeFrameWrite(HT08_PROFILE, NO_OPT_IN, frame);
    expect(result.ok).toBe(false);
    expect(denialOf(result)?.code).toBe("experimental-opt-in-required");
  });

  it("allows enabling an experimental opcode with opt-in", () => {
    const frame = encodeCommand(Cmd.SpatialAudio, [0x01]);
    expect(authorizeFrameWrite(HT08_PROFILE, OPTED_IN, frame).ok).toBe(true);
  });

  it("treats NoiseCancelMode 0x0C as experimental after live HT08 falsification", () => {
    // Live HT08 unit ignored 0x0C writes (2026-08-27); 0x17 is the confirmed
    // ANC path. Enabling ANC via 0x0C now requires the experimental opt-in...
    const enable = encodeCommand(Cmd.NoiseCancelMode, [0x01]);
    expect(authorizeFrameWrite(HT08_PROFILE, NO_OPT_IN, enable).ok).toBe(false);
    expect(denialOf(authorizeFrameWrite(HT08_PROFILE, NO_OPT_IN, enable))?.code)
      .toBe("experimental-opt-in-required");
    expect(authorizeFrameWrite(HT08_PROFILE, OPTED_IN, enable).ok).toBe(true);
    // 0x0C "off" is 0x00, which does not match the 0x02 pure-disable
    // convention, so it is gated the same way until re-evidenced.
    const disable = encodeCommand(Cmd.NoiseCancelMode, [0x00]);
    expect(authorizeFrameWrite(HT08_PROFILE, NO_OPT_IN, disable).ok).toBe(false);
    expect(authorizeFrameWrite(HT08_PROFILE, OPTED_IN, disable).ok).toBe(true);
  });

  it("allows disabling an experimental opcode without opt-in (safe cleanup)", () => {
    const frame = encodeCommand(Cmd.EnvAdaptation, [0x02]);
    expect(authorizeFrameWrite(HT08_PROFILE, NO_OPT_IN, frame).ok).toBe(true);
  });

  it("denies enabling experimental EnvAdaptation without opt-in but allows disabling it", () => {
    // 0x32 EnvAdaptation stays experimental in the ledger (unvalidated on the
    // live HT08; the validated adaptive path is 0x17 payload (1,5,2)). The
    // policy itself must still gate enable and allow safe disable.
    const enable = encodeCommand(Cmd.EnvAdaptation, [0x01]);
    expect(authorizeFrameWrite(HT08_PROFILE, NO_OPT_IN, enable).ok).toBe(false);
    const disable = encodeCommand(Cmd.EnvAdaptation, [0x02]);
    expect(authorizeFrameWrite(HT08_PROFILE, NO_OPT_IN, disable).ok).toBe(true);
  });
});

describe("central write policy · direct characteristic allowlist", () => {
  it("allows HT08 direct writes to keyFunctionV2 and eqDirect", () => {
    const keys = authorizeDirectWrite(
      HT08_PROFILE,
      NO_OPT_IN,
      CHAR.keyFunctionV2,
      Uint8Array.from([0x01, 0x01]),
    );
    const eq = authorizeDirectWrite(
      HT08_PROFILE,
      NO_OPT_IN,
      CHAR.eqDirect,
      Uint8Array.from([0x00]),
    );
    expect(keys.ok).toBe(true);
    expect(eq.ok).toBe(true);
  });

  it("denies HT08 direct writes to a non-allowlisted characteristic", () => {
    const result = authorizeDirectWrite(
      HT08_PROFILE,
      OPTED_IN,
      CHAR.settingsNotify,
      Uint8Array.from([0x00]),
    );
    expect(result.ok).toBe(false);
    expect(denialOf(result)?.code).toBe("direct-char-not-allowed");
  });
});

describe("central write policy · malformed input", () => {
  it("denies an undecodable frame", () => {
    const result = authorizeFrameWrite(
      HT08_PROFILE,
      OPTED_IN,
      Uint8Array.from([0x00, 0x01, 0x00]),
    );
    expect(result.ok).toBe(false);
    expect(denialOf(result)?.code).toBe("undecodable-frame");
  });
});

describe("transport enforcement · no bypass at the write boundary", () => {
  it("MockTransport.write rejects a raw destructive frame", async () => {
    const t = new MockTransport();
    await expect(t.write(encodeCommand(Cmd.FactoryReset, []))).rejects.toThrow(
      WriteDeniedError,
    );
  });

  it("MockTransport.write rejects destructive frames even when opted in", async () => {
    const t = new MockTransport();
    t.setExperimentalOptIn(true);
    await expect(t.write(encodeCommand(Cmd.ClearPairing, []))).rejects.toThrow(
      WriteDeniedError,
    );
  });

  it("MockTransport.write accepts an allowed supported write", async () => {
    const t = new MockTransport();
    await expect(t.write(encodeCommand(Cmd.LowLatency, [0x01]))).resolves.toBeUndefined();
  });

  it("MockTransport.write rejects enabling an experimental write without opt-in", async () => {
    const t = new MockTransport();
    await expect(t.write(encodeCommand(Cmd.SpatialAudio, [0x01]))).rejects.toThrow(
      WriteDeniedError,
    );
    t.setExperimentalOptIn(true);
    await expect(t.write(encodeCommand(Cmd.SpatialAudio, [0x01]))).resolves.toBeUndefined();
  });

  it("MockTransport.writeDirect rejects a non-allowlisted characteristic", async () => {
    const t = new MockTransport();
    await expect(
      t.writeDirect(CHAR.settingsNotify, Uint8Array.from([0x00])),
    ).rejects.toThrow(WriteDeniedError);
  });

  it("MockTransport.writeDirect accepts an allowlisted characteristic", async () => {
    const t = new MockTransport();
    await expect(
      t.writeDirect(CHAR.keyFunctionV2, Uint8Array.from([0x01, 0x01])),
    ).resolves.toBeUndefined();
  });
});
