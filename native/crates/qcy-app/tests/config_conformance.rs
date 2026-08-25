//! Shared config-schema conformance (issue #8 parity with the TypeScript #11
//! contract). Consumes `conformance/config_vectors.json`; the TypeScript side is
//! pinned by `src/lib/qcy/config-schema.conformance.test.ts`.

use qcy_app::config::*;
use serde_json::Value;

#[derive(serde::Deserialize)]
struct VectorFile {
    version: u32,
    valid: Vec<ValidVector>,
    invalid: Vec<InvalidVector>,
}

#[derive(serde::Deserialize)]
struct ValidVector {
    name: String,
    json: Value,
    expect: ValidExpect,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidExpect {
    theme: String,
    active_profile_id: String,
    auto_game: bool,
    auto_game_keyword: String,
    custom_eq_count: usize,
    custom_profiles_count: usize,
    notify_connected: bool,
    #[serde(default)]
    imported_profile_builtin: Option<bool>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct InvalidVector {
    name: String,
    json: Value,
    #[serde(default)]
    expect_error_path_prefix: Option<String>,
    #[serde(default)]
    stored_only: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    note: Option<String>,
}

fn vectors() -> VectorFile {
    const RAW: &str = include_str!("../../../../conformance/config_vectors.json");
    let file: VectorFile = serde_json::from_str(RAW).expect("config vector file parses");
    assert_eq!(file.version, 1);
    file
}

#[test]
fn valid_vectors_parse_with_expected_values() {
    for vector in vectors().valid {
        let parsed = parse_external_config(&vector.json)
            .unwrap_or_else(|errors| panic!("{}: expected success, got {errors:?}", vector.name));
        let theme = match parsed.theme {
            ThemeMode::Dark => "dark",
            ThemeMode::Light => "light",
        };
        assert_eq!(theme, vector.expect.theme, "{}: theme", vector.name);
        assert_eq!(
            parsed.active_profile_id, vector.expect.active_profile_id,
            "{}: activeProfileId",
            vector.name
        );
        assert_eq!(
            parsed.auto_game, vector.expect.auto_game,
            "{}: autoGame",
            vector.name
        );
        assert_eq!(
            parsed.auto_game_keyword, vector.expect.auto_game_keyword,
            "{}: autoGameKeyword",
            vector.name
        );
        assert_eq!(
            parsed.custom_eq.len(),
            vector.expect.custom_eq_count,
            "{}: customEq count",
            vector.name
        );
        assert_eq!(
            parsed.custom_profiles.len(),
            vector.expect.custom_profiles_count,
            "{}: customProfiles count",
            vector.name
        );
        assert_eq!(
            parsed.notify.connected, vector.expect.notify_connected,
            "{}: notify.connected",
            vector.name
        );
        if let Some(expected_builtin) = vector.expect.imported_profile_builtin {
            assert_eq!(
                parsed.custom_profiles[0].builtin, expected_builtin,
                "{}: builtin never trusted",
                vector.name
            );
        }
    }
}

#[test]
fn invalid_vectors_are_rejected_atomically() {
    for vector in vectors().invalid {
        let json = if vector.json == Value::String("OVER_LIMIT_MARKER".into()) {
            // Construct 65 valid profiles programmatically (awkward to store inline).
            let profiles: Vec<Value> = (0..65)
                .map(|i| {
                    serde_json::json!({
                        "id": format!("p{i}"), "name": "P", "description": "",
                        "noise": "off", "ancLevel": 1, "transparencyLevel": 1,
                        "gameMode": false, "eqId": "flat", "wearDetection": true
                    })
                })
                .collect();
            serde_json::json!({ "customProfiles": profiles })
        } else {
            vector.json.clone()
        };
        let result = if vector.stored_only.unwrap_or(false) {
            parse_any_stored_config(&json).map(|_| ())
        } else {
            parse_external_config(&json).map(|_| ())
        };
        let errors = match result {
            Ok(()) => panic!("{}: expected rejection, got success", vector.name),
            Err(errors) => errors,
        };
        if let Some(prefix) = &vector.expect_error_path_prefix {
            assert!(
                errors.iter().any(|e| e.path.starts_with(prefix)),
                "{}: expected an error under {prefix}, got {errors:?}",
                vector.name
            );
        }
    }
}

#[test]
fn export_import_round_trip_preserves_portable_values() {
    // Build a config with one custom EQ and one custom profile, export it, import it,
    // and compare the portable fields — the desktop/browser interchange contract.
    let mut external = default_external();
    external.custom_eq.push(NamedEq {
        id: "warm".into(),
        name: "Warm".into(),
        kind: "device".into(),
        official: false,
        preset: EqPreset {
            index: 0.0,
            master_gain_db: -1.0,
            bands: (0..limits::EQ_BAND_COUNT)
                .map(|i| EqBand {
                    freq_hz: 31.0 * (i as f64 + 1.0),
                    gain_db: i as f64 * 0.5,
                    q: 1.0,
                    band_type: None,
                })
                .collect(),
        },
    });
    let exported = build_export(&external);
    let imported = parse_import(&exported).expect("own export must import");
    assert_eq!(imported, external);
}
