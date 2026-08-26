import { describe, expect, it } from "vitest";
import {
  CONFIG_SCHEMA_VERSION,
  DEFAULT_CONFIG,
  LEGACY_STORAGE_KEY,
  LIMITS,
  STORAGE_KEY,
  buildExport,
  loadPersistedConfig,
  parseAnyStoredConfig,
  parseExternalConfig,
  parseImport,
  parsePersistedConfig,
  savePersistedConfig,
  summarizeErrors,
  type ConfigStorage,
  type PersistedConfig,
} from "./config-schema";

/* ------------------------------------------------------------------ */
/* Fixtures                                                            */
/* ------------------------------------------------------------------ */

function validEq(over: Record<string, unknown> = {}) {
  return {
    id: "myeq",
    name: "My EQ",
    kind: "device",
    official: false,
    preset: {
      index: 0,
      masterGainDb: 0,
      bands: Array.from({ length: LIMITS.eqBandCount }, (_, i) => ({
        freqHz: [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000][i],
        gainDb: 1.5,
        q: 1,
        bandType: 0,
      })),
    },
    ...over,
  };
}

function validProfile(over: Record<string, unknown> = {}) {
  return {
    id: "myprofile",
    name: "My Profile",
    description: "A test profile",
    builtin: false,
    noise: "anc",
    ancLevel: 2,
    transparencyLevel: 4,
    gameMode: false,
    eqId: "myeq",
    wearDetection: true,
    ...over,
  };
}

function validExternal(over: Record<string, unknown> = {}) {
  return {
    schema: CONFIG_SCHEMA_VERSION,
    theme: "dark",
    notify: { ...DEFAULT_CONFIG.notify },
    customEq: [validEq()],
    customProfiles: [validProfile()],
    activeProfileId: "myprofile",
    autoGame: false,
    autoGameKeyword: "game",
    ...over,
  };
}

function validPersisted(over: Record<string, unknown> = {}): PersistedConfig {
  return {
    ...(validExternal() as PersistedConfig),
    hideMac: true,
    sleepTimerMin: 30,
    lastSeen: { at: "2025-01-01T00:00:00.000Z", host: "this-computer", rssi: -52 },
    knownDevices: ["84:AC:60:62:69:DA"],
    ...over,
  };
}

class MemStorage implements ConfigStorage {
  map = new Map<string, string>();
  getItem(k: string) {
    return this.map.has(k) ? (this.map.get(k) as string) : null;
  }
  setItem(k: string, v: string) {
    this.map.set(k, v);
  }
}

/* ------------------------------------------------------------------ */
/* Round-trip                                                          */
/* ------------------------------------------------------------------ */

describe("config schema round-trip", () => {
  it("export -> import yields the same portable values", () => {
    const cfg = validPersisted();
    const exported = buildExport(cfg);
    const parsed = parseImport(exported);
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;
    expect(parsed.value.theme).toBe(cfg.theme);
    expect(parsed.value.notify).toEqual(cfg.notify);
    expect(parsed.value.customEq).toEqual(cfg.customEq);
    expect(parsed.value.customProfiles.length).toBe(1);
    expect(parsed.value.activeProfileId).toBe(cfg.activeProfileId);
    expect(parsed.value.schema).toBe(CONFIG_SCHEMA_VERSION);
  });

  it("save -> load round-trips the full persisted config", () => {
    const storage = new MemStorage();
    const cfg = validPersisted();
    savePersistedConfig(storage, cfg);
    const loaded = loadPersistedConfig(storage);
    expect(loaded.usedDefaults).toBe(false);
    expect(loaded.config).toEqual(cfg);
  });

  it("export never includes local-only or runtime fields", () => {
    const cfg = validPersisted();
    const exported = buildExport(cfg);
    expect(exported).not.toContain("hideMac");
    expect(exported).not.toContain("sleepTimerMin");
    expect(exported).not.toContain("lastSeen");
    expect(exported).not.toContain("this-computer");
    expect(exported).not.toContain("knownDevices");
    expect(exported).not.toContain("84:AC:60:62:69:DA");
  });
});

/* ------------------------------------------------------------------ */
/* Malformed input                                                     */
/* ------------------------------------------------------------------ */

describe("config schema malformed input", () => {
  it("rejects non-JSON", () => {
    const r = parseImport("{not json");
    expect(r.ok).toBe(false);
  });

  it("rejects non-object roots", () => {
    for (const bad of ["null", "42", '"str"', "[]"]) {
      const r = parseImport(bad);
      expect(r.ok).toBe(false);
    }
  });

  it("rejects wrong-typed fields with structured errors", () => {
    const r = parseExternalConfig(validExternal({ theme: "neon", autoGame: "yes" }));
    expect(r.ok).toBe(false);
    if (r.ok) return;
    const paths = r.errors.map((e) => e.path);
    expect(paths).toContain("theme");
    expect(paths).toContain("autoGame");
  });

  it("rejects an EQ preset with the wrong band count", () => {
    const eq = validEq();
    (eq.preset as { bands: unknown[] }).bands = (eq.preset as { bands: unknown[] }).bands.slice(0, 3);
    const r = parseExternalConfig(validExternal({ customEq: [eq] }));
    expect(r.ok).toBe(false);
  });

  it("rejects out-of-range gains", () => {
    const eq = validEq();
    (eq.preset as { bands: { gainDb: number }[] }).bands[0].gainDb = 99;
    const r = parseExternalConfig(validExternal({ customEq: [eq] }));
    expect(r.ok).toBe(false);
  });

  it("rejects an unknown noise mode in a profile", () => {
    const r = parseExternalConfig(validExternal({ customProfiles: [validProfile({ noise: "turbo" })] }));
    expect(r.ok).toBe(false);
  });

  it("ignores unknown extra keys rather than trusting them", () => {
    const r = parseExternalConfig(validExternal({ someFutureKey: 123 }));
    expect(r.ok).toBe(true);
    if (r.ok) expect("someFutureKey" in r.value).toBe(false);
  });
});

/* ------------------------------------------------------------------ */
/* Old versions / migration                                            */
/* ------------------------------------------------------------------ */

describe("config schema migration", () => {
  it("migrates a legacy schema-less payload (v0) to the current schema", () => {
    const legacy = {
      theme: "light",
      notify: DEFAULT_CONFIG.notify,
      hideMac: false,
      customEq: [],
      customProfiles: [],
      activeProfileId: "music",
      autoGame: true,
      autoGameKeyword: "play",
      sleepTimerMin: 45,
      lastSeen: null,
    };
    const r = parseAnyStoredConfig(legacy);
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.value.schema).toBe(CONFIG_SCHEMA_VERSION);
      expect(r.value.theme).toBe("light");
      expect(r.value.autoGame).toBe(true);
    }
  });

  it("loadPersistedConfig migrates the legacy storage key to the new key", () => {
    const storage = new MemStorage();
    const legacy = { theme: "light", notify: DEFAULT_CONFIG.notify };
    storage.setItem(LEGACY_STORAGE_KEY, JSON.stringify(legacy));
    const loaded = loadPersistedConfig(storage);
    expect(loaded.migrated).toBe(true);
    expect(loaded.config.theme).toBe("light");
    // Migrated payload is written under the new key with a schema version.
    const stored = JSON.parse(storage.getItem(STORAGE_KEY) as string);
    expect(stored.schema).toBe(CONFIG_SCHEMA_VERSION);
  });

  it("refuses to downgrade a payload from a newer schema version", () => {
    const future = validPersisted({ schema: CONFIG_SCHEMA_VERSION + 1 });
    const r = parseAnyStoredConfig(future);
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.errors[0].path).toBe("schema");
  });

  it("corrupt stored JSON falls back to defaults without destroying stored data", () => {
    const storage = new MemStorage();
    storage.setItem(STORAGE_KEY, "{corrupt");
    const loaded = loadPersistedConfig(storage);
    expect(loaded.usedDefaults).toBe(true);
    expect(loaded.errors.length).toBeGreaterThan(0);
    // Stored data is left in place, not overwritten.
    expect(storage.getItem(STORAGE_KEY)).toBe("{corrupt");
  });

  it("empty storage yields defaults", () => {
    const loaded = loadPersistedConfig(new MemStorage());
    expect(loaded.usedDefaults).toBe(true);
    expect(loaded.config.theme).toBe(DEFAULT_CONFIG.theme);
  });
});

/* ------------------------------------------------------------------ */
/* Boundaries                                                          */
/* ------------------------------------------------------------------ */

describe("config schema boundaries", () => {
  it("enforces the custom EQ count limit", () => {
    const tooMany = Array.from({ length: LIMITS.maxCustomEq + 1 }, (_, i) =>
      validEq({ id: `eq${i}` }),
    );
    const r = parseExternalConfig(validExternal({ customEq: tooMany }));
    expect(r.ok).toBe(false);
  });

  it("enforces the custom profile count limit", () => {
    const tooMany = Array.from({ length: LIMITS.maxCustomProfiles + 1 }, (_, i) =>
      validProfile({ id: `p${i}` }),
    );
    const r = parseExternalConfig(validExternal({ customProfiles: tooMany }));
    expect(r.ok).toBe(false);
  });

  it("enforces string length limits", () => {
    const r = parseExternalConfig(validExternal({ autoGameKeyword: "x".repeat(LIMITS.maxKeywordLen + 1) }));
    expect(r.ok).toBe(false);
  });

  it("enforces sleep timer bounds on persisted config", () => {
    expect(parsePersistedConfig(validPersisted({ sleepTimerMin: LIMITS.sleepTimerMax + 1 })).ok).toBe(false);
    expect(parsePersistedConfig(validPersisted({ sleepTimerMin: LIMITS.sleepTimerMin - 1 })).ok).toBe(false);
    expect(parsePersistedConfig(validPersisted({ sleepTimerMin: LIMITS.sleepTimerMin })).ok).toBe(true);
  });

  it("enforces anc/transparency level bounds", () => {
    expect(parseExternalConfig(validExternal({ customProfiles: [validProfile({ ancLevel: 9 })] })).ok).toBe(false);
    expect(
      parseExternalConfig(validExternal({ customProfiles: [validProfile({ transparencyLevel: 0 })] })).ok,
    ).toBe(false);
  });

  it("never trusts builtin=true from imported profiles", () => {
    const r = parseExternalConfig(validExternal({ customProfiles: [validProfile({ builtin: true })] }));
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.value.customProfiles[0].builtin).toBe(false);
  });
});

/* ------------------------------------------------------------------ */
/* Error summarization                                                 */
/* ------------------------------------------------------------------ */

describe("summarizeErrors", () => {
  it("joins and truncates errors", () => {
    const errors = [
      { path: "a", message: "m1" },
      { path: "b", message: "m2" },
      { path: "c", message: "m3" },
      { path: "d", message: "m4" },
    ];
    const s = summarizeErrors(errors, 2);
    expect(s).toContain("a: m1");
    expect(s).toContain("b: m2");
    expect(s).toContain("+2 more");
  });
});
