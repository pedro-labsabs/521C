import { readFileSync, writeFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { HT08_CAPABILITIES } from "./capabilities";

// Capability matrix conformance vector (issue #8).
//
// The HT08 capability truth matrix lives in TypeScript (this module's sibling
// `capabilities.ts`), and the native Rust UI needs the exact same truth model.
// The matrix is therefore exported as a shared conformance vector consumed by the
// Rust `qcy-device` crate, following the same pattern as protocol_vectors.json:
//
//   - TypeScript: this test pins the committed vector to the live matrix.
//   - Rust: native/crates/qcy-device parses the same file and mirrors the
//     derivation rules (isShown / isImplemented / isWritable / canInteract /
//     summarizeCapability), tested against the same expectations.
//
// Regenerate the vector after changing the matrix with:
//   UPDATE_VECTOR=1 npx vitest run src/lib/qcy/device/capabilities-vector.test.ts

const VECTOR_PATH = "conformance/capabilities_ht08.json";

function serialize(): string {
  return (
    JSON.stringify(
      {
        version: 1,
        model: "HT08",
        source: "src/lib/qcy/device/capabilities.ts",
        capabilities: HT08_CAPABILITIES,
      },
      null,
      2,
    ) + "\n"
  );
}

describe("capability matrix conformance vector", () => {
  it("matches conformance/capabilities_ht08.json", () => {
    if (process.env.UPDATE_VECTOR === "1") {
      writeFileSync(VECTOR_PATH, serialize());
      return;
    }
    const committed = readFileSync(VECTOR_PATH, "utf8");
    expect(serialize()).toBe(committed);
  });
});
