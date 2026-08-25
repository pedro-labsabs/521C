//! Shared capability-vector conformance (issue #8).
//!
//! Pins the committed `conformance/capabilities_ht08.json` (generated from the
//! TypeScript matrix) against the truth-model invariants that the native UI relies
//! on. The TypeScript side pins vector freshness in
//! `src/lib/qcy/device/capabilities-vector.test.ts`.

use qcy_device::*;

#[test]
fn ht08_vector_parses_with_the_documented_shape() {
    let profile = ht08_profile();
    assert_eq!(profile.model, "HT08");
    // The matrix size is pinned so accidental entry loss is visible; bump it
    // deliberately when the matrix grows.
    assert_eq!(profile.capabilities.len(), 42);
    for (id, cap) in &profile.capabilities {
        // Every entry must carry the four truths; summarize must never panic.
        let _ = summarize_capability(cap);
        assert!(!id.is_empty());
    }
}

#[test]
fn firmware_ota_is_forbidden_and_never_interactable() {
    let profile = ht08_profile();
    let ota = profile.get("firmwareOta").expect("firmwareOta entry");
    assert_eq!(ota.write, WriteReadiness::Forbidden);
    assert!(!can_interact(ota));
    assert!(!is_writable(ota));
    assert_eq!(summarize_capability(ota).label, "Forbidden");
}

#[test]
fn host_features_never_claim_device_support_or_writes() {
    let profile = ht08_profile();
    for id in ["systemEq", "autoGameMode", "codecStatus"] {
        let cap = profile.get(id).expect("host feature entry");
        assert_eq!(cap.hardware, EvidenceTruth::Unknown, "{id} hardware");
        assert_eq!(cap.protocol, EvidenceTruth::Unknown, "{id} protocol");
        assert_eq!(cap.write, WriteReadiness::ReadOnly, "{id} write");
        assert!(!can_interact(cap), "{id} must not be a device interaction");
        assert!(!is_writable(cap), "{id} must not be writable");
    }
}

#[test]
fn proven_ht08_controls_are_interactable() {
    let profile = ht08_profile();
    for id in ["ancOff", "ancOn", "gameMode", "deviceEq", "sleepMode"] {
        let cap = profile.get(id).expect("control entry");
        assert!(is_writable(cap), "{id} should be a supported write");
        assert!(can_interact(cap), "{id} should be interactable");
        assert_eq!(summarize_capability(cap).label, "Supported");
    }
}

#[test]
fn experimental_controls_require_opt_in_and_are_labelled() {
    let profile = ht08_profile();
    for id in ["ancAdaptive", "spatialAudio", "ldacToggle"] {
        let cap = profile.get(id).expect("experimental entry");
        assert!(is_experimental_write(cap), "{id} should be experimental");
        assert!(can_interact(cap), "{id} is interactable behind opt-in");
        assert!(!is_writable(cap), "{id} is not a plain supported write");
        assert_eq!(summarize_capability(cap).label, "Experimental");
    }
}

#[test]
fn read_only_status_capabilities_are_not_interactions() {
    let profile = ht08_profile();
    for id in [
        "batteryLeft",
        "batteryRight",
        "batteryCase",
        "firmware",
        "rssi",
    ] {
        let cap = profile.get(id).expect("status entry");
        assert_eq!(cap.write, WriteReadiness::ReadOnly, "{id}");
        assert!(is_implemented(cap), "{id} should be implemented");
        assert!(!can_interact(cap), "{id} is state, not a control");
        assert_eq!(summarize_capability(cap).label, "Read-only");
    }
}

#[test]
fn unknown_or_research_rows_stay_downgraded() {
    let profile = ht08_profile();
    let wind = profile.get("ancWind").expect("ancWind entry");
    assert!(!can_interact(wind));
    assert!(!is_implemented(wind));
    let gps = profile.get("findGps").expect("findGps entry");
    assert!(!is_shown(gps) || !can_interact(gps));
}
