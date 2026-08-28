import { describe, expect, it } from "vitest";
import { Cmd, DESTRUCTIVE_CMDS } from "./types";
import {
  destructiveOpcodes,
  directWriteUuids,
  experimentalWriteOpcodes,
  OPCODE_EVIDENCE,
  supportedWriteOpcodes,
  UUID_EVIDENCE,
} from "./evidence";
import { GENERIC_QCY_PROFILE, HT08_PROFILE } from "../device/catalog";
import { HT08_CAPABILITIES } from "../device/capabilities";

/**
 * Evidence governance: these tests are the review rule that keeps the write
 * surface honest. A new trusted write that lacks a sufficient evidence entry
 * fails here before it can reach the transport.
 */

const CMD_OPCODES: number[] = Object.values(Cmd);

describe("evidence ledger coverage", () => {
  it("records every opcode in the Cmd catalog", () => {
    for (const op of CMD_OPCODES) {
      expect(OPCODE_EVIDENCE.has(op), `missing evidence for 0x${op.toString(16)}`).toBe(true);
    }
  });

  it("has no ledger entries for opcodes outside the Cmd catalog", () => {
    for (const op of OPCODE_EVIDENCE.keys()) {
      expect(CMD_OPCODES, `ledger has uncatalogued 0x${op.toString(16)}`).toContain(op);
    }
  });

  it("marks exactly the documented destructive opcodes as destructive", () => {
    expect(destructiveOpcodes()).toEqual(new Set([0x01, 0x02, 0x03]));
    expect(destructiveOpcodes()).toEqual(new Set(DESTRUCTIVE_CMDS));
  });
});

describe("trusted writes require sufficient evidence", () => {
  it("every writable opcode is backed by protocol-doc or hardware-capture evidence", () => {
    for (const [op, e] of OPCODE_EVIDENCE) {
      if (e.trustLevel === "write-supported" || e.trustLevel === "write-experimental") {
        expect(
          e.evidenceClass === "protocol-doc" || e.evidenceClass === "hardware-capture",
          `0x${op.toString(16)} (${e.name}) is writable but evidence is ${e.evidenceClass}`,
        ).toBe(true);
      }
    }
  });

  it("no community-catalog or official-app opcode is writable", () => {
    for (const [op, e] of OPCODE_EVIDENCE) {
      if (e.evidenceClass === "community-catalog" || e.evidenceClass === "official-app") {
        expect(
          e.trustLevel === "catalog-only" || e.trustLevel === "read",
          `0x${op.toString(16)} (${e.name}) is ${e.evidenceClass} but trust=${e.trustLevel}`,
        ).toBe(true);
      }
    }
  });

  it("every writable opcode records the model it was evidenced on", () => {
    for (const [op, e] of OPCODE_EVIDENCE) {
      if (e.trustLevel === "write-supported" || e.trustLevel === "write-experimental") {
        expect(e.observedModel, `0x${op.toString(16)} lacks observedModel`).toBeTruthy();
      }
    }
  });
});

describe("write policy is derived from the ledger", () => {
  it("HT08 supported writes exactly match ledger write-supported opcodes", () => {
    expect(HT08_PROFILE.writePolicy.supportedOpcodes).toEqual(supportedWriteOpcodes());
  });

  it("HT08 experimental writes exactly match ledger write-experimental opcodes", () => {
    expect(HT08_PROFILE.writePolicy.experimentalOpcodes).toEqual(experimentalWriteOpcodes());
  });

  it("HT08 direct-write chars exactly match ledger direct-write UUIDs", () => {
    expect(HT08_PROFILE.writePolicy.directChars).toEqual(directWriteUuids());
  });

  it("no catalog-only opcode is writable on HT08", () => {
    const writable = new Set([
      ...HT08_PROFILE.writePolicy.supportedOpcodes,
      ...HT08_PROFILE.writePolicy.experimentalOpcodes,
    ]);
    for (const [op, e] of OPCODE_EVIDENCE) {
      if (e.trustLevel === "catalog-only") {
        expect(writable.has(op), `catalog-only 0x${op.toString(16)} is writable`).toBe(false);
      }
    }
  });

  it("generic profile stays read-only with an empty write surface", () => {
    expect(GENERIC_QCY_PROFILE.readOnly).toBe(true);
    expect(GENERIC_QCY_PROFILE.writePolicy.supportedOpcodes.size).toBe(0);
    expect(GENERIC_QCY_PROFILE.writePolicy.experimentalOpcodes.size).toBe(0);
    expect(GENERIC_QCY_PROFILE.writePolicy.directChars.size).toBe(0);
  });
});

describe("ledger <-> capability matrix cross-references (#69)", () => {
  it("every ledger capability reference is a real capability key", () => {
    const keys = new Set(Object.keys(HT08_CAPABILITIES));
    for (const [op, e] of OPCODE_EVIDENCE) {
      if (e.capability !== undefined) {
        expect(
          keys.has(e.capability),
          `0x${op.toString(16)} (${e.name}) points at unknown capability "${e.capability}"`,
        ).toBe(true);
      }
    }
  });

  it("every capability opcode reference is a real ledger entry", () => {
    for (const [key, cap] of Object.entries(HT08_CAPABILITIES)) {
      if (cap.opcode !== undefined) {
        expect(
          OPCODE_EVIDENCE.has(cap.opcode),
          `capability "${key}" points at uncatalogued opcode 0x${cap.opcode.toString(16)}`,
        ).toBe(true);
      }
    }
  });

  it("writable/experimental capabilities reference matching ledger trust levels", () => {
    // The audit's stale links (0x0C -> ancOn, 0x17 -> ancLevels, 0x32 ->
    // ancAdaptive) derived wrong readiness from the join; pin the directions
    // that must always agree.
    for (const [key, cap] of Object.entries(HT08_CAPABILITIES)) {
      if (cap.opcode === undefined) continue;
      const ledger = OPCODE_EVIDENCE.get(cap.opcode);
      if (!ledger) continue;
      if (cap.write === "writable") {
        expect(
          ledger.trustLevel,
          `capability "${key}" is writable but 0x${cap.opcode.toString(16)} is ${ledger.trustLevel}`,
        ).toBe("write-supported");
      }
      if (cap.write === "experimental") {
        expect(
          ledger.trustLevel,
          `capability "${key}" is experimental but 0x${cap.opcode.toString(16)} is ${ledger.trustLevel}`,
        ).toBe("write-experimental");
      }
      if (cap.write === "forbidden") {
        expect(
          ledger.trustLevel,
          `capability "${key}" is forbidden but 0x${cap.opcode.toString(16)} is ${ledger.trustLevel}`,
        ).toBe("destructive");
      }
    }
  });

  it("no falsified/unvalidated opcode link survives in the ledger (#69)", () => {
    // 0x0C is falsified on HT08 and 0x32 is unvalidated there: neither may
    // claim a capability that the matrix validates through 0x17.
    expect(OPCODE_EVIDENCE.get(0x0c)?.capability).toBeUndefined();
    expect(OPCODE_EVIDENCE.get(0x32)?.capability).toBeUndefined();
    // 0x17 backs the scene capabilities, not adjustable levels.
    expect(OPCODE_EVIDENCE.get(0x17)?.capability).not.toBe("ancLevels");
  });
});

describe("UUID evidence", () => {
  it("only direct-write UUIDs are exposed as direct-write chars", () => {
    for (const [uuid, e] of UUID_EVIDENCE) {
      if (e.role === "direct-write") {
        expect(directWriteUuids().has(uuid)).toBe(true);
      }
    }
    expect(directWriteUuids().size).toBeGreaterThan(0);
  });
});
