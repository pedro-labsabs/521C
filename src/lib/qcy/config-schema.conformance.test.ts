/**
 * Shared config-schema conformance (issue #8 parity with the Rust `qcy-app`
 * config module). Consumes `conformance/config_vectors.json`; the Rust side is
 * pinned by `native/crates/qcy-app/tests/config_conformance.rs`. Both
 * implementations must accept the valid payloads with the expected parsed values
 * and reject the invalid ones atomically, so browser and desktop persistence
 * cannot drift into incompatible schemas.
 */
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  parseAnyStoredConfig,
  parseExternalConfig,
  type ConfigError,
} from "./config-schema";

type ValidExpect = {
  theme: string;
  activeProfileId: string;
  autoGame: boolean;
  autoGameKeyword: string;
  customEqCount: number;
  customProfilesCount: number;
  notifyConnected: boolean;
  importedProfileBuiltin?: boolean;
};

type ValidVector = { name: string; json: unknown; expect: ValidExpect };
type InvalidVector = {
  name: string;
  json: unknown;
  expectErrorPathPrefix?: string;
  storedOnly?: boolean;
  note?: string;
};

type VectorFile = {
  version: number;
  valid: ValidVector[];
  invalid: InvalidVector[];
};

const vectors = JSON.parse(
  readFileSync(
    new URL("../../../conformance/config_vectors.json", import.meta.url),
    "utf8",
  ),
) as VectorFile;

function errorsOf(result: { ok: boolean; errors?: ConfigError[] }): ConfigError[] {
  return result.ok ? [] : (result.errors ?? []);
}

describe("shared config vectors", () => {
  it("pins the vector corpus version", () => {
    expect(vectors.version).toBe(1);
    expect(vectors.valid.length).toBeGreaterThanOrEqual(3);
    expect(vectors.invalid.length).toBeGreaterThanOrEqual(6);
  });

  for (const vector of vectors.valid) {
    it(`accepts: ${vector.name}`, () => {
      const result = parseExternalConfig(vector.json);
      expect(result.ok, JSON.stringify(errorsOf(result))).toBe(true);
      if (!result.ok) return;
      const config = result.value;
      expect(config.theme).toBe(vector.expect.theme);
      expect(config.activeProfileId).toBe(vector.expect.activeProfileId);
      expect(config.autoGame).toBe(vector.expect.autoGame);
      expect(config.autoGameKeyword).toBe(vector.expect.autoGameKeyword);
      expect(config.customEq).toHaveLength(vector.expect.customEqCount);
      expect(config.customProfiles).toHaveLength(vector.expect.customProfilesCount);
      expect(config.notify.connected).toBe(vector.expect.notifyConnected);
      if (vector.expect.importedProfileBuiltin !== undefined) {
        expect(config.customProfiles[0]?.builtin).toBe(
          vector.expect.importedProfileBuiltin,
        );
      }
    });
  }

  for (const vector of vectors.invalid) {
    it(`rejects: ${vector.name}`, () => {
      const json =
        vector.json === "OVER_LIMIT_MARKER"
          ? {
              // Construct 65 valid profiles programmatically (awkward inline).
              customProfiles: Array.from({ length: 65 }, (_, i) => ({
                id: `p${i}`,
                name: "P",
                description: "",
                noise: "off",
                ancLevel: 1,
                transparencyLevel: 1,
                gameMode: false,
                eqId: "flat",
                wearDetection: true,
              })),
            }
          : vector.json;
      const result = vector.storedOnly
        ? parseAnyStoredConfig(json)
        : parseExternalConfig(json);
      expect(result.ok).toBe(false);
      if (result.ok || !vector.expectErrorPathPrefix) return;
      expect(
        result.errors.some((e) =>
          e.path.startsWith(vector.expectErrorPathPrefix as string),
        ),
        JSON.stringify(result.errors),
      ).toBe(true);
    });
  }
});
