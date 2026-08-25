//! Versioned, validated configuration (issue #11 contract, Rust side of issue #8).
//!
//! This module is the Rust mirror of `src/lib/qcy/config-schema.ts`. Both sides share
//! one JSON contract (schema version travels inside the payload) so browser and
//! desktop persistence cannot drift into incompatible schemas. Validation behavior is
//! pinned by the shared corpus at `conformance/config_vectors.json`, consumed by both
//! test suites.
//!
//! Field classes (exactly as in the TypeScript contract):
//!
//!  1. portable   — backup/export files and local persistence (theme, notify, custom
//!     EQ, custom profiles, active profile, auto game-mode trigger);
//!  2. local-only — persisted on this machine, never exported (`hideMac`,
//!     `sleepTimerMin`, `lastSeen`);
//!  3. runtime    — session state, never persisted (connection state, opt-ins, logs).
//!
//! External data is never trusted: it is validated field-by-field before it can touch
//! application state, and invalid input is rejected atomically with structured errors.

use serde::{Deserialize, Serialize};

pub const CONFIG_SCHEMA_VERSION: u32 = 1;

/// Limits — identical to `LIMITS` in `src/lib/qcy/config-schema.ts`.
pub mod limits {
    pub const MAX_CUSTOM_EQ: usize = 64;
    pub const MAX_CUSTOM_PROFILES: usize = 64;
    pub const MAX_ID_LEN: usize = 64;
    pub const MAX_NAME_LEN: usize = 80;
    pub const MAX_DESCRIPTION_LEN: usize = 280;
    pub const MAX_KEYWORD_LEN: usize = 64;
    pub const EQ_BAND_COUNT: usize = 10;
    pub const GAIN_MIN: f64 = -12.0;
    pub const GAIN_MAX: f64 = 12.0;
    pub const ANC_LEVEL_MIN: i64 = 1;
    pub const ANC_LEVEL_MAX: i64 = 3;
    pub const TRANSPARENCY_LEVEL_MIN: i64 = 1;
    pub const TRANSPARENCY_LEVEL_MAX: i64 = 7;
    pub const SLEEP_TIMER_MIN: i64 = 5;
    pub const SLEEP_TIMER_MAX: i64 = 240;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoiseUiMode {
    Off,
    Anc,
    Adaptive,
    Indoor,
    Commuting,
    Noisy,
    Transparency,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotifyPrefs {
    pub connected: bool,
    pub disconnected: bool,
    pub battery_low: bool,
    pub battery_critical: bool,
    pub battery_uneven: bool,
    pub profile_switch: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqBand {
    pub freq_hz: f64,
    pub gain_db: f64,
    pub q: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub band_type: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqPreset {
    #[serde(default)]
    pub index: f64,
    pub master_gain_db: f64,
    pub bands: Vec<EqBand>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedEq {
    pub id: String,
    pub name: String,
    pub kind: String, // "device" | "system"
    pub official: bool,
    pub preset: EqPreset,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Imported/persisted profiles are user profiles; builtin status is never trusted
    /// from external data.
    pub builtin: bool,
    pub noise: NoiseUiMode,
    pub anc_level: i64,
    pub transparency_level: i64,
    pub game_mode: bool,
    pub eq_id: String,
    pub wear_detection: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_app: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LastSeen {
    pub at: String,
    pub host: String,
    pub rssi: f64,
}

/// Portable fields — safe for backup/export and local persistence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalConfig {
    pub schema: u32,
    pub theme: ThemeMode,
    pub notify: NotifyPrefs,
    pub custom_eq: Vec<NamedEq>,
    pub custom_profiles: Vec<SmartProfile>,
    pub active_profile_id: String,
    pub auto_game: bool,
    pub auto_game_keyword: String,
}

/// Local persistence = portable fields + host-specific fields that are never exported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedConfig {
    #[serde(flatten)]
    pub external: ExternalConfig,
    pub hide_mac: bool,
    pub sleep_timer_min: i64,
    pub last_seen: Option<LastSeen>,
}

pub fn default_notify() -> NotifyPrefs {
    NotifyPrefs {
        connected: true,
        disconnected: true,
        battery_low: true,
        battery_critical: true,
        battery_uneven: true,
        profile_switch: true,
    }
}

pub fn default_external() -> ExternalConfig {
    ExternalConfig {
        schema: CONFIG_SCHEMA_VERSION,
        theme: ThemeMode::Dark,
        notify: default_notify(),
        custom_eq: Vec::new(),
        custom_profiles: Vec::new(),
        active_profile_id: "music".into(),
        auto_game: false,
        auto_game_keyword: "game".into(),
    }
}

pub fn default_persisted() -> PersistedConfig {
    PersistedConfig {
        external: default_external(),
        hide_mac: true,
        sleep_timer_min: 30,
        last_seen: None,
    }
}

/* ------------------------------------------------------------------ */
/* Structured validation errors                                        */
/* ------------------------------------------------------------------ */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub path: String,
    pub message: String,
}

pub type ParseResult<T> = Result<T, Vec<ConfigError>>;

struct Validator {
    errors: Vec<ConfigError>,
}

impl Validator {
    fn new() -> Self {
        Self { errors: Vec::new() }
    }
    fn fail(&mut self, path: &str, message: impl Into<String>) {
        self.errors.push(ConfigError {
            path: path.to_string(),
            message: message.into(),
        });
    }
    fn finish<T>(self, value: T) -> ParseResult<T> {
        if self.errors.is_empty() {
            Ok(value)
        } else {
            Err(self.errors)
        }
    }
}

fn as_object<'a>(
    v: &'a serde_json::Value,
    path: &str,
    validator: &mut Validator,
) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
    match v {
        serde_json::Value::Object(map) => Some(map),
        _ => {
            validator.fail(path, "expected a JSON object");
            None
        }
    }
}

fn get_bool(
    map: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    path: &str,
    validator: &mut Validator,
    fallback: bool,
) -> Option<bool> {
    match map.get(key) {
        None | Some(serde_json::Value::Null) => Some(fallback),
        Some(serde_json::Value::Bool(b)) => Some(*b),
        Some(_) => {
            validator.fail(&format!("{path}.{key}"), "expected a boolean");
            None
        }
    }
}

fn get_string(
    map: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    path: &str,
    validator: &mut Validator,
    max_len: usize,
    fallback: &str,
) -> Option<String> {
    match map.get(key) {
        None | Some(serde_json::Value::Null) => Some(fallback.to_string()),
        Some(serde_json::Value::String(s)) => {
            if s.len() > max_len {
                validator.fail(
                    &format!("{path}.{key}"),
                    format!("longer than {max_len} characters"),
                );
                None
            } else {
                Some(s.clone())
            }
        }
        Some(_) => {
            validator.fail(&format!("{path}.{key}"), "expected a string");
            None
        }
    }
}

fn get_finite(map: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<Option<f64>> {
    // None => absent; Some(None) => present but not a finite number.
    map.get(key).map(|v| v.as_f64())
}

fn get_int_in_range(
    map: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    path: &str,
    validator: &mut Validator,
    min: i64,
    max: i64,
    fallback: i64,
) -> Option<i64> {
    match map.get(key) {
        None | Some(serde_json::Value::Null) => Some(fallback),
        Some(v) => match v.as_i64() {
            Some(i) if i >= min && i <= max => Some(i),
            _ => {
                validator.fail(
                    &format!("{path}.{key}"),
                    format!("must be an integer between {min} and {max}"),
                );
                None
            }
        },
    }
}

fn validate_eq_band(v: &mut Validator, path: &str, value: &serde_json::Value) -> Option<EqBand> {
    let map = as_object(value, path, v)?;
    let mut ok = true;
    let freq = match get_finite(map, "freqHz") {
        Some(Some(f)) if f > 0.0 => f,
        Some(_) => {
            v.fail(&format!("{path}.freqHz"), "must be a positive number");
            ok = false;
            0.0
        }
        None => {
            v.fail(&format!("{path}.freqHz"), "must be a positive number");
            ok = false;
            0.0
        }
    };
    let gain = match get_finite(map, "gainDb") {
        Some(Some(g)) if (limits::GAIN_MIN..=limits::GAIN_MAX).contains(&g) => g,
        Some(_) => {
            v.fail(
                &format!("{path}.gainDb"),
                format!(
                    "must be a number between {} and {}",
                    limits::GAIN_MIN,
                    limits::GAIN_MAX
                ),
            );
            ok = false;
            0.0
        }
        None => {
            v.fail(
                &format!("{path}.gainDb"),
                format!(
                    "must be a number between {} and {}",
                    limits::GAIN_MIN,
                    limits::GAIN_MAX
                ),
            );
            ok = false;
            0.0
        }
    };
    let q = match get_finite(map, "q") {
        Some(Some(q)) if q > 0.0 => q,
        Some(_) => {
            v.fail(&format!("{path}.q"), "must be a positive number");
            ok = false;
            0.0
        }
        None => {
            v.fail(&format!("{path}.q"), "must be a positive number");
            ok = false;
            0.0
        }
    };
    let band_type = match map.get("bandType") {
        None | Some(serde_json::Value::Null) => None,
        Some(b) => match b.as_f64() {
            Some(f) => Some(f),
            None => {
                v.fail(&format!("{path}.bandType"), "must be a number");
                ok = false;
                None
            }
        },
    };
    if !ok {
        return None;
    }
    Some(EqBand {
        freq_hz: freq,
        gain_db: gain,
        q,
        band_type,
    })
}

fn validate_eq_preset(
    v: &mut Validator,
    path: &str,
    value: &serde_json::Value,
) -> Option<EqPreset> {
    let map = as_object(value, path, v)?;
    let mut ok = true;
    let master = match get_finite(map, "masterGainDb") {
        Some(Some(g)) if (limits::GAIN_MIN..=limits::GAIN_MAX).contains(&g) => g,
        _ => {
            v.fail(
                &format!("{path}.masterGainDb"),
                format!(
                    "must be between {} and {}",
                    limits::GAIN_MIN,
                    limits::GAIN_MAX
                ),
            );
            ok = false;
            0.0
        }
    };
    let bands = match map.get("bands").and_then(|b| b.as_array()) {
        Some(arr) if arr.len() == limits::EQ_BAND_COUNT => {
            let mut out = Vec::with_capacity(limits::EQ_BAND_COUNT);
            for (i, b) in arr.iter().enumerate() {
                match validate_eq_band(v, &format!("{path}.bands[{i}]"), b) {
                    Some(band) => out.push(band),
                    None => ok = false,
                }
            }
            out
        }
        _ => {
            v.fail(
                &format!("{path}.bands"),
                format!(
                    "must be an array of exactly {} bands",
                    limits::EQ_BAND_COUNT
                ),
            );
            ok = false;
            Vec::new()
        }
    };
    let index = map.get("index").and_then(|i| i.as_f64()).unwrap_or(0.0);
    if !ok {
        return None;
    }
    Some(EqPreset {
        index,
        master_gain_db: master,
        bands,
    })
}

fn validate_named_eq(v: &mut Validator, path: &str, value: &serde_json::Value) -> Option<NamedEq> {
    let map = as_object(value, path, v)?;
    let mut ok = true;
    let id = match get_string(map, "id", path, v, limits::MAX_ID_LEN, "") {
        Some(s) => s,
        None => {
            ok = false;
            String::new()
        }
    };
    let name = match get_string(map, "name", path, v, limits::MAX_NAME_LEN, "") {
        Some(s) => s,
        None => {
            ok = false;
            String::new()
        }
    };
    let kind = match map.get("kind").and_then(|k| k.as_str()) {
        Some("device") => "device".to_string(),
        Some("system") => "system".to_string(),
        _ => {
            v.fail(&format!("{path}.kind"), "must be one of: device, system");
            ok = false;
            String::new()
        }
    };
    let official = match get_bool(map, "official", path, v, false) {
        Some(b) => b,
        None => {
            ok = false;
            false
        }
    };
    let preset = match map.get("preset") {
        Some(p) => match validate_eq_preset(v, &format!("{path}.preset"), p) {
            Some(p) => p,
            None => {
                ok = false;
                EqPreset {
                    index: 0.0,
                    master_gain_db: 0.0,
                    bands: Vec::new(),
                }
            }
        },
        None => {
            v.fail(&format!("{path}.preset"), "expected an object");
            ok = false;
            EqPreset {
                index: 0.0,
                master_gain_db: 0.0,
                bands: Vec::new(),
            }
        }
    };
    if !ok || id.is_empty() || name.is_empty() {
        return None;
    }
    Some(NamedEq {
        id,
        name,
        kind,
        official,
        preset,
    })
}

fn validate_noise_mode(
    v: &mut Validator,
    path: &str,
    value: Option<&serde_json::Value>,
) -> Option<NoiseUiMode> {
    let parsed = value.and_then(|n| n.as_str()).and_then(|s| match s {
        "off" => Some(NoiseUiMode::Off),
        "anc" => Some(NoiseUiMode::Anc),
        "adaptive" => Some(NoiseUiMode::Adaptive),
        "indoor" => Some(NoiseUiMode::Indoor),
        "commuting" => Some(NoiseUiMode::Commuting),
        "noisy" => Some(NoiseUiMode::Noisy),
        "transparency" => Some(NoiseUiMode::Transparency),
        _ => None,
    });
    match parsed {
        Some(mode) => Some(mode),
        None => {
            v.fail(
                path,
                "must be one of: off, anc, adaptive, indoor, commuting, noisy, transparency",
            );
            None
        }
    }
}

fn validate_smart_profile(
    v: &mut Validator,
    path: &str,
    value: &serde_json::Value,
) -> Option<SmartProfile> {
    let map = as_object(value, path, v)?;
    let mut ok = true;
    let id = match get_string(map, "id", path, v, limits::MAX_ID_LEN, "") {
        Some(s) => s,
        None => {
            ok = false;
            String::new()
        }
    };
    let name = match get_string(map, "name", path, v, limits::MAX_NAME_LEN, "") {
        Some(s) => s,
        None => {
            ok = false;
            String::new()
        }
    };
    let description = match get_string(map, "description", path, v, limits::MAX_DESCRIPTION_LEN, "")
    {
        Some(s) => s,
        None => {
            ok = false;
            String::new()
        }
    };
    let noise = match validate_noise_mode(v, &format!("{path}.noise"), map.get("noise")) {
        Some(n) => n,
        None => {
            ok = false;
            NoiseUiMode::Off
        }
    };
    let anc_level = match get_int_in_range(
        map,
        "ancLevel",
        path,
        v,
        limits::ANC_LEVEL_MIN,
        limits::ANC_LEVEL_MAX,
        0,
    ) {
        Some(l) => l,
        None => {
            ok = false;
            0
        }
    };
    let transparency_level = match get_int_in_range(
        map,
        "transparencyLevel",
        path,
        v,
        limits::TRANSPARENCY_LEVEL_MIN,
        limits::TRANSPARENCY_LEVEL_MAX,
        0,
    ) {
        Some(l) => l,
        None => {
            ok = false;
            0
        }
    };
    let game_mode = match get_bool(map, "gameMode", path, v, false) {
        Some(b) => b,
        None => {
            ok = false;
            false
        }
    };
    let wear_detection = match get_bool(map, "wearDetection", path, v, false) {
        Some(b) => b,
        None => {
            ok = false;
            false
        }
    };
    let eq_id = match get_string(map, "eqId", path, v, limits::MAX_ID_LEN, "") {
        Some(s) => s,
        None => {
            ok = false;
            String::new()
        }
    };
    let trigger_app = match map.get("triggerApp") {
        None | Some(serde_json::Value::Null) => None,
        Some(_) => match get_string(map, "triggerApp", path, v, limits::MAX_NAME_LEN, "") {
            Some(s) => Some(s),
            None => {
                ok = false;
                None
            }
        },
    };
    if !ok || id.is_empty() || name.is_empty() {
        return None;
    }
    Some(SmartProfile {
        id,
        name,
        description,
        builtin: false, // never trusted from external data
        noise,
        anc_level,
        transparency_level,
        game_mode,
        eq_id,
        wear_detection,
        trigger_app,
    })
}

fn validate_notify(
    v: &mut Validator,
    path: &str,
    value: Option<&serde_json::Value>,
) -> Option<NotifyPrefs> {
    let defaults = default_notify();
    let Some(value) = value else {
        return Some(defaults);
    };
    let map = as_object(value, path, v)?;
    let mut ok = true;
    let mut out = defaults;
    for (key, slot) in [
        ("connected", &mut out.connected),
        ("disconnected", &mut out.disconnected),
        ("batteryLow", &mut out.battery_low),
        ("batteryCritical", &mut out.battery_critical),
        ("batteryUneven", &mut out.battery_uneven),
        ("profileSwitch", &mut out.profile_switch),
    ] {
        match get_bool(map, key, path, v, *slot) {
            Some(b) => *slot = b,
            None => ok = false,
        }
    }
    if ok {
        Some(out)
    } else {
        None
    }
}

fn validate_last_seen(
    v: &mut Validator,
    path: &str,
    value: Option<&serde_json::Value>,
) -> Option<Option<LastSeen>> {
    let Some(value) = value else {
        return Some(None);
    };
    if value.is_null() {
        return Some(None);
    }
    let map = as_object(value, path, v)?;
    let mut ok = true;
    let at = match get_string(map, "at", path, v, 64, "") {
        Some(s) => s,
        None => {
            ok = false;
            String::new()
        }
    };
    let host = match get_string(map, "host", path, v, 128, "") {
        Some(s) => s,
        None => {
            ok = false;
            String::new()
        }
    };
    let rssi = match map.get("rssi").and_then(|r| r.as_f64()) {
        Some(r) if (-127.0..=127.0).contains(&r) => r,
        _ => {
            v.fail(
                &format!("{path}.rssi"),
                "must be a number between -127 and 127",
            );
            ok = false;
            0.0
        }
    };
    if !ok || at.is_empty() || host.is_empty() {
        return None;
    }
    Some(Some(LastSeen { at, host, rssi }))
}

/* ------------------------------------------------------------------ */
/* Whole-object parsing                                                */
/* ------------------------------------------------------------------ */

/// Validate the portable (exportable) fields. Unknown extra keys are ignored, never
/// trusted. Missing fields take documented defaults; wrong-typed fields reject the
/// whole payload atomically.
pub fn parse_external_config(raw: &serde_json::Value) -> ParseResult<ExternalConfig> {
    let mut v = Validator::new();
    let defaults = default_external();
    let Some(map) = as_object(raw, "$", &mut v) else {
        return v.finish(defaults);
    };

    let theme = match map.get("theme") {
        None => defaults.theme,
        Some(t) => match t.as_str() {
            Some("dark") => ThemeMode::Dark,
            Some("light") => ThemeMode::Light,
            _ => {
                v.fail("theme", "must be one of: dark, light");
                defaults.theme
            }
        },
    };
    let notify = match validate_notify(&mut v, "notify", map.get("notify")) {
        Some(n) => n,
        None => {
            v.finish(defaults)?;
            unreachable!()
        }
    };
    let active_profile_id = match get_string(
        map,
        "activeProfileId",
        "",
        &mut v,
        limits::MAX_ID_LEN,
        &defaults.active_profile_id,
    ) {
        Some(s) => s,
        None => defaults.active_profile_id.clone(),
    };
    let auto_game = match get_bool(map, "autoGame", "", &mut v, defaults.auto_game) {
        Some(b) => b,
        None => defaults.auto_game,
    };
    let auto_game_keyword = match get_string(
        map,
        "autoGameKeyword",
        "",
        &mut v,
        limits::MAX_KEYWORD_LEN,
        &defaults.auto_game_keyword,
    ) {
        Some(s) => s,
        None => defaults.auto_game_keyword.clone(),
    };

    let mut custom_eq = Vec::new();
    match map.get("customEq") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::Array(arr)) => {
            if arr.len() > limits::MAX_CUSTOM_EQ {
                v.fail(
                    "customEq",
                    format!("more than {} entries", limits::MAX_CUSTOM_EQ),
                );
            } else {
                for (i, item) in arr.iter().enumerate() {
                    if let Some(eq) = validate_named_eq(&mut v, &format!("customEq[{i}]"), item) {
                        custom_eq.push(eq);
                    }
                }
            }
        }
        Some(_) => v.fail("customEq", "expected an array"),
    }

    let mut custom_profiles = Vec::new();
    match map.get("customProfiles") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::Array(arr)) => {
            if arr.len() > limits::MAX_CUSTOM_PROFILES {
                v.fail(
                    "customProfiles",
                    format!("more than {} entries", limits::MAX_CUSTOM_PROFILES),
                );
            } else {
                for (i, item) in arr.iter().enumerate() {
                    if let Some(p) =
                        validate_smart_profile(&mut v, &format!("customProfiles[{i}]"), item)
                    {
                        custom_profiles.push(p);
                    }
                }
            }
        }
        Some(_) => v.fail("customProfiles", "expected an array"),
    }

    if !v.errors.is_empty() {
        return Err(v.errors);
    }
    Ok(ExternalConfig {
        schema: CONFIG_SCHEMA_VERSION,
        theme,
        notify,
        custom_eq,
        custom_profiles,
        active_profile_id,
        auto_game,
        auto_game_keyword,
    })
}

/// Validate a full persisted payload (portable + local-only fields).
pub fn parse_persisted_config(raw: &serde_json::Value) -> ParseResult<PersistedConfig> {
    let external = parse_external_config(raw)?;
    let mut v = Validator::new();
    let defaults = default_persisted();
    let Some(map) = as_object(raw, "$", &mut v) else {
        return Err(v.errors);
    };
    let mut ok = true;
    let hide_mac = match get_bool(map, "hideMac", "", &mut v, defaults.hide_mac) {
        Some(b) => b,
        None => {
            ok = false;
            defaults.hide_mac
        }
    };
    let sleep_timer_min = match get_int_in_range(
        map,
        "sleepTimerMin",
        "",
        &mut v,
        limits::SLEEP_TIMER_MIN,
        limits::SLEEP_TIMER_MAX,
        defaults.sleep_timer_min,
    ) {
        Some(t) => t,
        None => {
            ok = false;
            defaults.sleep_timer_min
        }
    };
    let last_seen = match validate_last_seen(&mut v, "lastSeen", map.get("lastSeen")) {
        Some(l) => l,
        None => {
            ok = false;
            None
        }
    };
    if !ok {
        return Err(v.errors);
    }
    Ok(PersistedConfig {
        external,
        hide_mac,
        sleep_timer_min,
        last_seen,
    })
}

/// Parse a stored or imported payload of any known generation:
///
///  * `{ "schema": 1, ... }`  — validated directly;
///  * object without `schema` — legacy browser payload (pre-issue #11), validated with
///    the same rules (field names are unchanged);
///  * `{ "schema": >1 }`      — written by a newer app version; rejected without
///    touching the stored data.
pub fn parse_any_stored_config(raw: &serde_json::Value) -> ParseResult<PersistedConfig> {
    if let Some(schema) = raw.get("schema").and_then(|s| s.as_u64()) {
        if schema > u64::from(CONFIG_SCHEMA_VERSION) {
            return Err(vec![ConfigError {
                path: "schema".into(),
                message: format!(
                    "config schema v{schema} is newer than this app (v{CONFIG_SCHEMA_VERSION}); refusing to downgrade"
                ),
            }]);
        }
    }
    parse_persisted_config(raw)
}

/* ------------------------------------------------------------------ */
/* Storage                                                             */
/* ------------------------------------------------------------------ */

/// Minimal storage boundary (filesystem in production, in-memory in tests).
pub trait ConfigStorage {
    fn read(&self) -> Option<String>;
    fn write(&mut self, value: &str);
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadResult {
    pub config: PersistedConfig,
    /// True when the payload was rejected or absent and defaults were used.
    pub used_defaults: bool,
    pub errors: Vec<ConfigError>,
}

/// Load, validate and (when needed) fall back to defaults. Corrupt or newer payloads
/// fall back to defaults WITHOUT overwriting what is stored, so data is never
/// destroyed.
pub fn load_persisted_config(storage: &mut dyn ConfigStorage) -> LoadResult {
    let Some(raw_text) = storage.read() else {
        return LoadResult {
            config: default_persisted(),
            used_defaults: true,
            errors: Vec::new(),
        };
    };
    let parsed: serde_json::Value = match serde_json::from_str(&raw_text) {
        Ok(v) => v,
        Err(_) => {
            return LoadResult {
                config: default_persisted(),
                used_defaults: true,
                errors: vec![ConfigError {
                    path: "$".into(),
                    message: "stored config is not valid JSON; using defaults".into(),
                }],
            }
        }
    };
    match parse_any_stored_config(&parsed) {
        Ok(config) => LoadResult {
            config,
            used_defaults: false,
            errors: Vec::new(),
        },
        Err(errors) => LoadResult {
            config: default_persisted(),
            used_defaults: true,
            errors,
        },
    }
}

/// Persist the full local config (portable + local-only fields) with the schema
/// version.
pub fn save_persisted_config(storage: &mut dyn ConfigStorage, config: &PersistedConfig) {
    storage.write(&serde_json::to_string_pretty(config).expect("PersistedConfig serializes"));
}

/// Build the export/backup payload: portable fields only. Local-only fields and
/// runtime state are deliberately excluded by the privacy contract.
pub fn build_export(config: &ExternalConfig) -> String {
    let portable = ExternalConfig {
        schema: CONFIG_SCHEMA_VERSION,
        theme: config.theme,
        notify: config.notify.clone(),
        custom_eq: config.custom_eq.clone(),
        custom_profiles: config.custom_profiles.clone(),
        active_profile_id: config.active_profile_id.clone(),
        auto_game: config.auto_game,
        auto_game_keyword: config.auto_game_keyword.clone(),
    };
    serde_json::to_string_pretty(&portable).expect("ExternalConfig serializes")
}

/// Parse and validate an import file atomically.
pub fn parse_import(json: &str) -> ParseResult<ExternalConfig> {
    let parsed: serde_json::Value = serde_json::from_str(json).map_err(|_| {
        vec![ConfigError {
            path: "$".into(),
            message: "not valid JSON".into(),
        }]
    })?;
    if let Some(schema) = parsed.get("schema").and_then(|s| s.as_u64()) {
        if schema > u64::from(CONFIG_SCHEMA_VERSION) {
            return Err(vec![ConfigError {
                path: "schema".into(),
                message: format!("file uses config schema v{schema}, newer than this app (v{CONFIG_SCHEMA_VERSION})"),
            }]);
        }
    }
    parse_external_config(&parsed)
}

/// Human-readable one-line summary of validation errors (for toasts/UI).
pub fn summarize_errors(errors: &[ConfigError], max: usize) -> String {
    let shown: Vec<String> = errors
        .iter()
        .take(max)
        .map(|e| format!("{}: {}", e.path, e.message))
        .collect();
    let mut out = shown.join("; ");
    if errors.len() > max {
        out.push_str(&format!(" (+{} more)", errors.len() - max));
    }
    out
}

/* ------------------------------------------------------------------ */
/* XDG filesystem storage                                              */
/* ------------------------------------------------------------------ */

/// Filesystem-backed storage at the 521C XDG config path.
pub struct XdgStorage {
    path: std::path::PathBuf,
}

impl XdgStorage {
    /// `~/.config/521c/config.json`, honoring `XDG_CONFIG_HOME`.
    pub fn default_path() -> Option<std::path::PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
            })?;
        Some(base.join("521c").join("config.json"))
    }

    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl ConfigStorage for XdgStorage {
    fn read(&self) -> Option<String> {
        std::fs::read_to_string(&self.path).ok()
    }
    fn write(&mut self, value: &str) {
        if let Some(parent) = self.path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return;
            }
        }
        // Atomic replace: write a temp file in the same directory, then
        // rename over the target. A crash mid-write can therefore never
        // truncate the existing config; the old file stays intact until
        // the new one is fully on disk.
        let tmp = self.path.with_extension("json.tmp");
        if std::fs::write(&tmp, value).is_err() {
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        if std::fs::rename(&tmp, &self.path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MemStorage(Option<String>);
    impl ConfigStorage for MemStorage {
        fn read(&self) -> Option<String> {
            self.0.clone()
        }
        fn write(&mut self, value: &str) {
            self.0 = Some(value.to_string());
        }
    }

    #[test]
    fn save_load_round_trips_the_full_persisted_config() {
        let mut storage = MemStorage::default();
        let mut config = default_persisted();
        config.external.theme = ThemeMode::Light;
        config.sleep_timer_min = 45;
        config.last_seen = Some(LastSeen {
            at: "2026-08-25T12:00:00Z".into(),
            host: "mint".into(),
            rssi: -52.0,
        });
        save_persisted_config(&mut storage, &config);
        let loaded = load_persisted_config(&mut storage);
        assert!(!loaded.used_defaults);
        assert_eq!(loaded.config, config);
    }

    #[test]
    fn export_never_includes_local_only_or_runtime_fields() {
        let mut config = default_persisted();
        config.hide_mac = false;
        config.last_seen = Some(LastSeen {
            at: "x".into(),
            host: "y".into(),
            rssi: -40.0,
        });
        let export = build_export(&config.external);
        assert!(!export.contains("hideMac"));
        assert!(!export.contains("sleepTimerMin"));
        assert!(!export.contains("lastSeen"));
        assert!(export.contains("\"schema\": 1"));
    }

    #[test]
    fn corrupt_stored_json_falls_back_without_destroying_data() {
        let mut storage = MemStorage(Some("{ not json".into()));
        let loaded = load_persisted_config(&mut storage);
        assert!(loaded.used_defaults);
        assert_eq!(loaded.config, default_persisted());
        // Stored data untouched.
        assert_eq!(storage.0.as_deref(), Some("{ not json"));
    }

    #[test]
    fn newer_schema_is_refused() {
        let json = r#"{"schema": 2, "theme": "dark"}"#;
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
        let err = parse_any_stored_config(&parsed).unwrap_err();
        assert!(err[0].message.contains("newer than this app"));
    }

    #[test]
    fn import_rejects_out_of_range_gain_atomically() {
        let band = r#"{"freqHz": 100, "gainDb": 99, "q": 1}"#;
        let json = format!(
            r#"{{"customEq": [{{"id": "a", "name": "A", "kind": "device", "official": false, "preset": {{"masterGainDb": 0, "bands": [{band},{band},{band},{band},{band},{band},{band},{band},{band},{band}]}}}}]}}"#
        );
        let errors = parse_import(&json).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.path.starts_with("customEq[0].preset.bands")));
    }

    #[test]
    fn xdg_default_path_honors_xdg_config_home() {
        // Exercise the pure path logic without assuming the test host's HOME.
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/521c-xdg-test");
        let path = XdgStorage::default_path().unwrap();
        assert_eq!(
            path,
            std::path::PathBuf::from("/tmp/521c-xdg-test/521c/config.json")
        );
        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    fn xdg_storage_write_is_atomic_and_leaves_no_temp_file() {
        let dir = std::env::temp_dir().join(format!(
            "521c-cfg-atomic-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let path = dir.join("config.json");
        let mut storage = XdgStorage::new(path.clone());

        // First write creates the file with the exact content.
        storage.write(r#"{"schema":1}"#);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), r#"{"schema":1}"#);

        // Overwrite replaces atomically; no stray temp file remains.
        storage.write(r#"{"schema":1,"theme":"dark"}"#);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"schema":1,"theme":"dark"}"#
        );
        assert!(
            !dir.join("config.json.tmp").exists(),
            "temp file must not outlive a successful write"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
