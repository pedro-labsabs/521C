/**
 * Versioned, validated configuration schema (issue #11).
 *
 * 521C configuration crosses three boundaries, and each field belongs to exactly one:
 *
 *  1. EXPORT ("portable")   — backup/export files and local persistence. Safe to move
 *     between machines: theme, notifications, custom EQ, custom profiles, active
 *     profile, auto game-mode trigger.
 *  2. LOCAL_ONLY            — persisted on this machine but never exported, because it
 *     is host-specific or privacy-sensitive: `hideMac`, `sleepTimerMin`, `lastSeen`,
 *     `knownDevices`.
 *  3. RUNTIME_ONLY          — session/live state, never persisted: connection state,
 *     telemetry, log, toasts, experimental write opt-in, pending chime, etc.
 *
 * External data (localStorage payloads and import files) is never trusted: it is
 * validated field-by-field against this schema before it can touch application state.
 * Invalid input is rejected atomically with structured errors — no partial mutation.
 *
 * The schema version travels INSIDE the data (`schema` field), not in the storage key,
 * so future versions can migrate instead of relying on key suffixes. The native desktop
 * persistence planned in issue #8 should reuse this same JSON contract.
 */

import type { EqBand, EqPreset } from "./protocol/types";
import type { NamedEq } from "./eq-presets";
import type { NoiseUiMode, SmartProfile } from "./smart-profiles";

export const CONFIG_SCHEMA_VERSION = 1;

/** Current storage key. The schema version lives inside the payload, not the key. */
export const STORAGE_KEY = "521c-config";
/** Legacy key written before issue #11; migrated on load, kept as a fallback. */
export const LEGACY_STORAGE_KEY = "521c-config-v1";

export type ThemeMode = "dark" | "light";

export type NotifyPrefs = {
  connected: boolean;
  disconnected: boolean;
  batteryLow: boolean;
  batteryCritical: boolean;
  batteryUneven: boolean;
  profileSwitch: boolean;
};

export type LastSeen = { at: string; host: string; rssi: number } | null;

/** Portable fields — safe for backup/export and local persistence. */
export type ExternalConfig = {
  schema: typeof CONFIG_SCHEMA_VERSION;
  theme: ThemeMode;
  notify: NotifyPrefs;
  customEq: NamedEq[];
  customProfiles: SmartProfile[];
  activeProfileId: string;
  autoGame: boolean;
  autoGameKeyword: string;
};

/** Local persistence = portable fields + host-specific fields that are never exported. */
export type PersistedConfig = ExternalConfig & {
  hideMac: boolean;
  sleepTimerMin: number;
  lastSeen: LastSeen;
  /**
   * Addresses whose model the user explicitly confirmed (e.g. a renamed HT08).
   * Local-only: Bluetooth addresses are privacy-sensitive and never exported.
   */
  knownDevices: string[];
};

/* ------------------------------------------------------------------ */
/* Limits                                                              */
/* ------------------------------------------------------------------ */

/**
 * Length limits are UTF-8 BYTES (not UTF-16 code units), matching the Rust
 * validator so both sides of the shared config contract agree (issue #71).
 */
export const LIMITS = {
  maxCustomEq: 64,
  maxCustomProfiles: 64,
  maxIdLen: 64,
  maxNameLen: 80,
  maxDescriptionLen: 280,
  maxKeywordLen: 64,
  eqBandCount: 10,
  gainMin: -12,
  gainMax: 12,
  ancLevelMin: 1,
  ancLevelMax: 3,
  transparencyLevelMin: 1,
  transparencyLevelMax: 7,
  sleepTimerMin: 5,
  sleepTimerMax: 240,
  maxKnownDevices: 16,
  maxAddressLen: 32,
} as const;

export const DEFAULT_CONFIG: Omit<PersistedConfig, "schema"> = {
  theme: "dark",
  notify: {
    connected: true,
    disconnected: true,
    batteryLow: true,
    batteryCritical: true,
    batteryUneven: true,
    profileSwitch: true,
  },
  customEq: [],
  customProfiles: [],
  activeProfileId: "music",
  autoGame: false,
  autoGameKeyword: "game",
  hideMac: true,
  sleepTimerMin: 30,
  lastSeen: null,
  knownDevices: [],
};

/* ------------------------------------------------------------------ */
/* Result types                                                        */
/* ------------------------------------------------------------------ */

export type ConfigError = { path: string; message: string };
export type ParseResult<T> =
  | { ok: true; value: T }
  | { ok: false; errors: ConfigError[] };

/* ------------------------------------------------------------------ */
/* Small validators                                                    */
/* ------------------------------------------------------------------ */

// Must stay in sync with the NoiseUiMode union (smart-profiles.ts). The
// round-trip test in config-schema.test.ts iterates every union member, so a
// new mode that is not added here fails the suite instead of silently
// rejecting persisted configs at load time (issue #63: a "wind" profile used
// to wipe the entire persisted config).
const NOISE_MODES: readonly NoiseUiMode[] = [
  "off",
  "anc",
  "adaptive",
  "indoor",
  "commuting",
  "noisy",
  "wind",
  "transparency",
];

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

const utf8Encoder = new TextEncoder();

/**
 * UTF-8 byte length of a string. String limits in this schema are measured in
 * UTF-8 bytes so the TypeScript and Rust validators agree on the same shared
 * config contract (issue #71): a UTF-16 code-unit count rejects different
 * non-ASCII inputs near the limit than the Rust side's byte count.
 */
function utf8Length(v: string): number {
  return utf8Encoder.encode(v).length;
}

function isFiniteNumber(v: unknown): v is number {
  return typeof v === "number" && Number.isFinite(v);
}

class Validator {
  errors: ConfigError[] = [];

  fail(path: string, message: string): void {
    this.errors.push({ path, message });
  }

  boolean(path: string, v: unknown, fallback?: boolean): boolean | undefined {
    if (typeof v === "boolean") return v;
    if (v === undefined && fallback !== undefined) return fallback;
    if (v !== undefined) this.fail(path, "expected a boolean");
    return undefined;
  }

  string(path: string, v: unknown, maxLen: number): string | undefined {
    if (typeof v !== "string") {
      if (v !== undefined) this.fail(path, "expected a string");
      return undefined;
    }
    if (utf8Length(v) > maxLen) {
      this.fail(path, `longer than ${maxLen} bytes (UTF-8)`);
      return undefined;
    }
    return v;
  }

  intInRange(
    path: string,
    v: unknown,
    min: number,
    max: number,
    fallback?: number,
  ): number | undefined {
    if (!isFiniteNumber(v)) {
      if (v === undefined && fallback !== undefined) return fallback;
      if (v !== undefined) this.fail(path, "expected a number");
      return undefined;
    }
    if (!Number.isInteger(v) || v < min || v > max) {
      this.fail(path, `must be an integer between ${min} and ${max}`);
      return undefined;
    }
    return v;
  }

  enum<T extends string>(path: string, v: unknown, allowed: readonly T[]): T | undefined {
    if (typeof v === "string" && (allowed as readonly string[]).includes(v)) return v as T;
    if (v !== undefined) this.fail(path, `must be one of: ${allowed.join(", ")}`);
    return undefined;
  }

  result<T>(value: T): ParseResult<T> {
    return this.errors.length === 0
      ? { ok: true, value }
      : { ok: false, errors: this.errors };
  }
}

function positiveNumber(v: Validator, path: string, val: unknown): number | undefined {
  if (!isFiniteNumber(val) || val <= 0) {
    v.fail(path, "must be a positive number");
    return undefined;
  }
  return val;
}

function numberInRange(
  v: Validator,
  path: string,
  val: unknown,
  min: number,
  max: number,
): number | undefined {
  if (!isFiniteNumber(val) || val < min || val > max) {
    v.fail(path, `must be a number between ${min} and ${max}`);
    return undefined;
  }
  return val;
}

function validateEqBand(v: Validator, path: string, band: unknown): EqBand | undefined {
  if (!isRecord(band)) {
    v.fail(path, "expected an object");
    return undefined;
  }
  const freqHz = positiveNumber(v, `${path}.freqHz`, band.freqHz);
  const gainDb = numberInRange(v, `${path}.gainDb`, band.gainDb, LIMITS.gainMin, LIMITS.gainMax);
  const q = positiveNumber(v, `${path}.q`, band.q);
  if (freqHz === undefined || gainDb === undefined || q === undefined) return undefined;
  const out: EqBand = { freqHz, gainDb, q };
  if (band.bandType !== undefined) {
    if (!isFiniteNumber(band.bandType)) {
      v.fail(`${path}.bandType`, "must be a number");
      return undefined;
    }
    out.bandType = band.bandType;
  }
  return out;
}

function validateEqPreset(v: Validator, path: string, preset: unknown): EqPreset | undefined {
  if (!isRecord(preset)) {
    v.fail(path, "expected an object");
    return undefined;
  }
  const masterGainDb = preset.masterGainDb;
  if (!isFiniteNumber(masterGainDb) || masterGainDb < LIMITS.gainMin || masterGainDb > LIMITS.gainMax) {
    v.fail(`${path}.masterGainDb`, `must be between ${LIMITS.gainMin} and ${LIMITS.gainMax}`);
    return undefined;
  }
  if (!Array.isArray(preset.bands) || preset.bands.length !== LIMITS.eqBandCount) {
    v.fail(`${path}.bands`, `must be an array of exactly ${LIMITS.eqBandCount} bands`);
    return undefined;
  }
  const bands: EqBand[] = [];
  let ok = true;
  preset.bands.forEach((b, i) => {
    const band = validateEqBand(v, `${path}.bands[${i}]`, b);
    if (band) bands.push(band);
    else ok = false;
  });
  if (!ok) return undefined;
  const index = isFiniteNumber(preset.index) ? preset.index : 0;
  return { index, masterGainDb, bands };
}

function validateNamedEq(v: Validator, path: string, item: unknown): NamedEq | undefined {
  if (!isRecord(item)) {
    v.fail(path, "expected an object");
    return undefined;
  }
  const id = v.string(`${path}.id`, item.id, LIMITS.maxIdLen);
  const name = v.string(`${path}.name`, item.name, LIMITS.maxNameLen);
  const kind = v.enum(`${path}.kind`, item.kind, ["device", "system"] as const);
  const official = v.boolean(`${path}.official`, item.official, false);
  const preset = validateEqPreset(v, `${path}.preset`, item.preset);
  if (id === undefined || name === undefined || kind === undefined || preset === undefined || official === undefined) {
    return undefined;
  }
  return { id, name, kind, official, preset };
}

function validateSmartProfile(v: Validator, path: string, item: unknown): SmartProfile | undefined {
  if (!isRecord(item)) {
    v.fail(path, "expected an object");
    return undefined;
  }
  const id = v.string(`${path}.id`, item.id, LIMITS.maxIdLen);
  const name = v.string(`${path}.name`, item.name, LIMITS.maxNameLen);
  const description = v.string(`${path}.description`, item.description, LIMITS.maxDescriptionLen);
  const noise = v.enum(`${path}.noise`, item.noise, NOISE_MODES);
  const ancLevel = v.intInRange(`${path}.ancLevel`, item.ancLevel, LIMITS.ancLevelMin, LIMITS.ancLevelMax);
  const transparencyLevel = v.intInRange(
    `${path}.transparencyLevel`,
    item.transparencyLevel,
    LIMITS.transparencyLevelMin,
    LIMITS.transparencyLevelMax,
  );
  const gameMode = v.boolean(`${path}.gameMode`, item.gameMode);
  const wearDetection = v.boolean(`${path}.wearDetection`, item.wearDetection);
  const eqId = v.string(`${path}.eqId`, item.eqId, LIMITS.maxIdLen);
  if (
    id === undefined ||
    name === undefined ||
    description === undefined ||
    noise === undefined ||
    ancLevel === undefined ||
    transparencyLevel === undefined ||
    gameMode === undefined ||
    wearDetection === undefined ||
    eqId === undefined
  ) {
    return undefined;
  }
  const out: SmartProfile = {
    id,
    name,
    description,
    // Imported/persisted profiles are user profiles; builtin status is never trusted
    // from external data.
    builtin: false,
    noise,
    ancLevel,
    transparencyLevel,
    gameMode,
    eqId,
    wearDetection,
  };
  if (item.triggerApp !== undefined) {
    const triggerApp = v.string(`${path}.triggerApp`, item.triggerApp, LIMITS.maxNameLen);
    if (triggerApp === undefined) return undefined;
    out.triggerApp = triggerApp;
  }
  return out;
}

function validateNotify(v: Validator, path: string, raw: unknown): NotifyPrefs | undefined {
  if (!isRecord(raw)) {
    v.fail(path, "expected an object");
    return undefined;
  }
  const keys = [
    "connected",
    "disconnected",
    "batteryLow",
    "batteryCritical",
    "batteryUneven",
    "profileSwitch",
  ] as const;
  const out = {} as NotifyPrefs;
  let ok = true;
  for (const k of keys) {
    const b = v.boolean(`${path}.${k}`, raw[k], DEFAULT_CONFIG.notify[k]);
    if (b === undefined) ok = false;
    else out[k] = b;
  }
  return ok ? out : undefined;
}

function validateLastSeen(v: Validator, path: string, raw: unknown): LastSeen | undefined {
  if (raw === null || raw === undefined) return null;
  if (!isRecord(raw)) {
    v.fail(path, "expected null or an object");
    return undefined;
  }
  const at = v.string(`${path}.at`, raw.at, 64);
  const host = v.string(`${path}.host`, raw.host, 128);
  const rssi = raw.rssi;
  if (at === undefined || host === undefined) return undefined;
  if (!isFiniteNumber(rssi) || rssi < -127 || rssi > 127) {
    v.fail(`${path}.rssi`, "must be a number between -127 and 127");
    return undefined;
  }
  return { at, host, rssi };
}

/**
 * Validate the local-only `knownDevices` list (addresses whose model the user
 * explicitly confirmed). Missing/null means an empty list; anything malformed
 * rejects the whole payload atomically.
 */
function validateKnownDevices(v: Validator, raw: unknown): string[] | undefined {
  if (raw === null || raw === undefined) return [];
  if (!Array.isArray(raw)) {
    v.fail("knownDevices", "expected an array of addresses");
    return undefined;
  }
  if (raw.length > LIMITS.maxKnownDevices) {
    v.fail("knownDevices", `more than ${LIMITS.maxKnownDevices} addresses`);
    return undefined;
  }
  const out: string[] = [];
  let ok = true;
  raw.forEach((item, i) => {
    if (typeof item !== "string") {
      v.fail(`knownDevices[${i}]`, "expected a string");
      ok = false;
      return;
    }
    const trimmed = item.trim();
    if (trimmed.length === 0) {
      v.fail(`knownDevices[${i}]`, "must not be empty");
      ok = false;
      return;
    }
    if (utf8Length(trimmed) > LIMITS.maxAddressLen) {
      v.fail(`knownDevices[${i}]`, `longer than ${LIMITS.maxAddressLen} bytes (UTF-8)`);
      ok = false;
      return;
    }
    out.push(trimmed);
  });
  return ok ? out : undefined;
}

/* ------------------------------------------------------------------ */
/* Whole-object parsing                                                */
/* ------------------------------------------------------------------ */

/**
 * Validate the portable (exportable) fields. Used for both import files and the
 * portable subset of persisted data. Unknown extra keys are ignored, never trusted.
 */
export function parseExternalConfig(raw: unknown): ParseResult<ExternalConfig> {
  const v = new Validator();
  if (!isRecord(raw)) {
    v.fail("$", "expected a JSON object");
    return v.result(null as never);
  }
  const theme = v.enum("theme", raw.theme, ["dark", "light"] as const) ?? DEFAULT_CONFIG.theme;
  const notify = validateNotify(v, "notify", raw.notify ?? DEFAULT_CONFIG.notify);
  const activeProfileId =
    v.string("activeProfileId", raw.activeProfileId, LIMITS.maxIdLen) ?? DEFAULT_CONFIG.activeProfileId;
  const autoGame = v.boolean("autoGame", raw.autoGame, DEFAULT_CONFIG.autoGame);
  const autoGameKeyword =
    v.string("autoGameKeyword", raw.autoGameKeyword, LIMITS.maxKeywordLen) ?? DEFAULT_CONFIG.autoGameKeyword;

  const customEq: NamedEq[] = [];
  if (raw.customEq !== undefined) {
    if (!Array.isArray(raw.customEq)) {
      v.fail("customEq", "expected an array");
    } else if (raw.customEq.length > LIMITS.maxCustomEq) {
      v.fail("customEq", `more than ${LIMITS.maxCustomEq} entries`);
    } else {
      raw.customEq.forEach((item, i) => {
        const eq = validateNamedEq(v, `customEq[${i}]`, item);
        if (eq) customEq.push(eq);
      });
    }
  }

  const customProfiles: SmartProfile[] = [];
  if (raw.customProfiles !== undefined) {
    if (!Array.isArray(raw.customProfiles)) {
      v.fail("customProfiles", "expected an array");
    } else if (raw.customProfiles.length > LIMITS.maxCustomProfiles) {
      v.fail("customProfiles", `more than ${LIMITS.maxCustomProfiles} entries`);
    } else {
      raw.customProfiles.forEach((item, i) => {
        const p = validateSmartProfile(v, `customProfiles[${i}]`, item);
        if (p) customProfiles.push(p);
      });
    }
  }

  if (v.errors.length > 0 || notify === undefined || autoGame === undefined) {
    return { ok: false, errors: v.errors.length > 0 ? v.errors : [{ path: "$", message: "invalid config" }] };
  }
  return v.result({
    schema: CONFIG_SCHEMA_VERSION,
    theme,
    notify,
    customEq,
    customProfiles,
    activeProfileId,
    autoGame,
    autoGameKeyword,
  });
}

/** Validate a full persisted payload (portable + local-only fields). */
export function parsePersistedConfig(raw: unknown): ParseResult<PersistedConfig> {
  const external = parseExternalConfig(raw);
  if (!external.ok) return external;
  const v = new Validator();
  const record = raw as Record<string, unknown>;
  const hideMac = v.boolean("hideMac", record.hideMac, DEFAULT_CONFIG.hideMac);
  const sleepTimerMin = v.intInRange(
    "sleepTimerMin",
    record.sleepTimerMin,
    LIMITS.sleepTimerMin,
    LIMITS.sleepTimerMax,
    DEFAULT_CONFIG.sleepTimerMin,
  );
  const lastSeen = validateLastSeen(v, "lastSeen", record.lastSeen);
  const knownDevices = validateKnownDevices(v, record.knownDevices);
  if (
    hideMac === undefined ||
    sleepTimerMin === undefined ||
    lastSeen === undefined ||
    knownDevices === undefined
  ) {
    return { ok: false, errors: v.errors };
  }
  return v.result({ ...external.value, hideMac, sleepTimerMin, lastSeen, knownDevices });
}

/* ------------------------------------------------------------------ */
/* Migration + storage                                                 */
/* ------------------------------------------------------------------ */

/**
 * Parse a stored or imported payload of any known generation.
 *
 *  - `{ schema: 1, ... }`           → validated directly.
 *  - object without `schema`        → legacy v0 payload (pre-issue #11), migrated.
 *  - `{ schema: >1 }`               → written by a newer app version; rejected without
 *                                      touching the stored data.
 */
export function parseAnyStoredConfig(raw: unknown): ParseResult<PersistedConfig> {
  if (isRecord(raw) && typeof raw.schema === "number" && raw.schema > CONFIG_SCHEMA_VERSION) {
    return {
      ok: false,
      errors: [
        {
          path: "schema",
          message: `config schema v${raw.schema} is newer than this app (v${CONFIG_SCHEMA_VERSION}); refusing to downgrade`,
        },
      ],
    };
  }
  return parsePersistedConfig(raw);
}

export interface ConfigStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export type LoadResult = {
  config: PersistedConfig;
  /** True when the payload was rejected and defaults were used instead. */
  usedDefaults: boolean;
  /** True when a legacy (schema-less) payload was migrated. */
  migrated: boolean;
  errors: ConfigError[];
};

/**
 * Load, validate and (when needed) migrate persisted config. Corrupt or newer payloads
 * fall back to defaults WITHOUT overwriting what is stored, so data is never destroyed.
 */
export function loadPersistedConfig(storage: ConfigStorage | undefined): LoadResult {
  const full: PersistedConfig = { schema: CONFIG_SCHEMA_VERSION, ...DEFAULT_CONFIG };
  if (!storage) return { config: full, usedDefaults: true, migrated: false, errors: [] };

  let rawText: string | null = null;
  let migrated = false;
  try {
    rawText = storage.getItem(STORAGE_KEY);
    if (rawText === null) {
      rawText = storage.getItem(LEGACY_STORAGE_KEY);
      migrated = rawText !== null;
    }
  } catch {
    return { config: full, usedDefaults: true, migrated: false, errors: [] };
  }
  if (rawText === null) return { config: full, usedDefaults: true, migrated: false, errors: [] };

  let parsedJson: unknown;
  try {
    parsedJson = JSON.parse(rawText);
  } catch {
    return {
      config: full,
      usedDefaults: true,
      migrated: false,
      errors: [{ path: "$", message: "stored config is not valid JSON; using defaults" }],
    };
  }

  const result = parseAnyStoredConfig(parsedJson);
  if (!result.ok) {
    return { config: full, usedDefaults: true, migrated: false, errors: result.errors };
  }

  if (migrated) {
    // Persist the migrated payload under the new key; the legacy key is left in place
    // as a fallback rather than deleted.
    try {
      storage.setItem(STORAGE_KEY, JSON.stringify(result.value));
    } catch {
      /* quota — non-fatal */
    }
  }
  return { config: result.value, usedDefaults: false, migrated, errors: [] };
}

/** Persist the full local config (portable + local-only fields) with the schema version. */
export function savePersistedConfig(storage: ConfigStorage | undefined, config: PersistedConfig): void {
  if (!storage) return;
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify(config));
  } catch {
    /* quota — non-fatal */
  }
}

/* ------------------------------------------------------------------ */
/* Export / import                                                     */
/* ------------------------------------------------------------------ */

/**
 * Build the export/backup payload: portable fields only. Local-only fields (`hideMac`,
 * `sleepTimerMin`) and runtime state (`lastSeen`, device identifiers, logs) are
 * deliberately excluded by the privacy contract.
 */
export function buildExport(config: ExternalConfig): string {
  // Explicitly pick the portable fields. Never spread the input: callers may pass a
  // full PersistedConfig, and local-only/runtime fields must not leak into backups.
  const portable: ExternalConfig = {
    schema: CONFIG_SCHEMA_VERSION,
    theme: config.theme,
    notify: config.notify,
    customEq: config.customEq,
    customProfiles: config.customProfiles,
    activeProfileId: config.activeProfileId,
    autoGame: config.autoGame,
    autoGameKeyword: config.autoGameKeyword,
  };
  return JSON.stringify(portable, null, 2);
}

/**
 * Parse and validate an import file atomically. Returns structured errors on any
 * problem (bad JSON, wrong shape, out-of-range values); callers must only apply the
 * result when `ok` is true.
 */
export function parseImport(json: string): ParseResult<ExternalConfig> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    return { ok: false, errors: [{ path: "$", message: "not valid JSON" }] };
  }
  if (isRecord(parsed) && typeof parsed.schema === "number" && parsed.schema > CONFIG_SCHEMA_VERSION) {
    return {
      ok: false,
      errors: [
        {
          path: "schema",
          message: `file uses config schema v${parsed.schema}, newer than this app (v${CONFIG_SCHEMA_VERSION})`,
        },
      ],
    };
  }
  return parseExternalConfig(parsed);
}

/** Human-readable one-line summary of validation errors (for toasts/UI). */
export function summarizeErrors(errors: ConfigError[], max = 3): string {
  const shown = errors.slice(0, max).map((e) => `${e.path}: ${e.message}`);
  const more = errors.length - shown.length;
  return shown.join("; ") + (more > 0 ? ` (+${more} more)` : "");
}
