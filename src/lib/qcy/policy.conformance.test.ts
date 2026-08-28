import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { HT08_PROFILE } from "./device/catalog";
import { DESTRUCTIVE_CMDS } from "./protocol";
import {
  destructiveOpcodes,
  directWriteUuids,
  experimentalWriteOpcodes,
  supportedWriteOpcodes,
} from "./protocol/evidence";

/**
 * Cross-language write-policy parity pin (issue #59).
 *
 * conformance/protocol_vectors.json `writePolicy.ht08` mirrors the canonical
 * evidence ledger and is the shared artifact both policy implementations pin
 * against (the Rust side in native/crates/qcy-transport). It exists because
 * the Rust allowlist once drifted from the ledger's #53 demotion of 0x0C
 * without either suite noticing. This test is read-only on the corpus: if the
 * ledger changes, the corpus must change deliberately, and both language
 * suites fail until they agree again.
 */

type WritePolicyVectors = {
  writePolicy: {
    ht08: {
      supported: string[];
      experimental: string[];
      destructive: string[];
      directChars: string[];
    };
  };
};

const corpus = JSON.parse(
  readFileSync("conformance/protocol_vectors.json", "utf8"),
) as WritePolicyVectors;

function toHexSet(ops: Iterable<number>): Set<string> {
  return new Set([...ops].map((o) => o.toString(16).padStart(2, "0")));
}

describe("TS write policy parity pin against the conformance corpus (#59)", () => {
  const ht08 = corpus.writePolicy.ht08;

  it("supported opcode set == corpus supported", () => {
    expect(toHexSet(supportedWriteOpcodes())).toEqual(new Set(ht08.supported));
    // The shipped HT08 profile must enforce exactly the ledger-derived set.
    expect(HT08_PROFILE.writePolicy.supportedOpcodes).toEqual(supportedWriteOpcodes());
  });

  it("experimental opcode set == corpus experimental", () => {
    expect(toHexSet(experimentalWriteOpcodes())).toEqual(new Set(ht08.experimental));
    expect(HT08_PROFILE.writePolicy.experimentalOpcodes).toEqual(experimentalWriteOpcodes());
  });

  it("destructive opcode set == corpus destructive", () => {
    expect(toHexSet(destructiveOpcodes())).toEqual(new Set(ht08.destructive));
    expect(toHexSet(DESTRUCTIVE_CMDS)).toEqual(new Set(ht08.destructive));
  });

  it("direct-write characteristic set == corpus directChars", () => {
    const ledgerChars = new Set([...directWriteUuids()].map((u) => u.toLowerCase()));
    expect(ledgerChars).toEqual(new Set(ht08.directChars.map((u) => u.toLowerCase())));
    const profileChars = new Set(
      [...HT08_PROFILE.writePolicy.directChars].map((u) => u.toLowerCase()),
    );
    expect(profileChars).toEqual(new Set(ht08.directChars.map((u) => u.toLowerCase())));
  });

  it("the three opcode classes are disjoint", () => {
    const supported = supportedWriteOpcodes();
    const experimental = experimentalWriteOpcodes();
    const destructive = destructiveOpcodes();
    for (const op of supported) {
      expect(experimental.has(op), `0x${op.toString(16)} in supported and experimental`).toBe(false);
      expect(destructive.has(op), `0x${op.toString(16)} in supported and destructive`).toBe(false);
    }
    for (const op of experimental) {
      expect(destructive.has(op), `0x${op.toString(16)} in experimental and destructive`).toBe(false);
    }
  });
});
