//! Device profiles and the capability truth model (issue #3, Rust mirror).
//!
//! The canonical HT08 capability matrix lives in TypeScript at
//! `src/lib/qcy/device/capabilities.ts`. It is exported as the shared conformance
//! vector `conformance/capabilities_ht08.json` (same corpus pattern as
//! `protocol_vectors.json`) and parsed here, so the native UI and the web surface
//! cannot drift apart: a matrix change fails the TypeScript freshness test until the
//! vector is regenerated, and this crate always reads the regenerated vector.
//!
//! Every capability answers four independent questions instead of one ambiguous
//! state, exactly like the TypeScript model:
//!
//!   * hardware       – is the feature associated with the hardware/model?
//!   * protocol       – is the protocol behavior evidenced for this model/firmware?
//!   * implementation – does this build actually implement it?
//!   * write          – is the operation writable / experimental / read-only / forbidden?
//!
//! The derivation rules below mirror `src/lib/qcy/device/capabilities.ts`
//! (`isShown`, `isImplemented`, `isWritable`, `isExperimentalWrite`, `canInteract`,
//! `summarizeCapability`) one-for-one.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Hardware/protocol evidence dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceTruth {
    Supported,
    Unknown,
    Unsupported,
}

/// Implementation dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImplTruth {
    Implemented,
    MockOnly,
    NotImplemented,
}

/// Write-readiness dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WriteReadiness {
    Writable,
    Experimental,
    ReadOnly,
    Forbidden,
}

/// One capability's four truths plus optional provenance note and opcode reference.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CapabilityTruth {
    pub hardware: EvidenceTruth,
    pub protocol: EvidenceTruth,
    pub implementation: ImplTruth,
    pub write: WriteReadiness,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub opcode: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct VectorFile {
    version: u32,
    model: String,
    #[allow(dead_code)]
    source: String,
    capabilities: BTreeMap<String, CapabilityTruth>,
}

/// A device profile: model id plus its capability matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceProfile {
    pub model: String,
    pub capabilities: BTreeMap<String, CapabilityTruth>,
}

/// Current shared-vector schema version this crate understands.
pub const VECTOR_VERSION: u32 = 1;

/// Parse a capability vector document. Rejects unknown schema versions instead of
/// guessing.
pub fn parse_vector(json: &str) -> Result<DeviceProfile, String> {
    let file: VectorFile =
        serde_json::from_str(json).map_err(|e| format!("invalid capability vector: {e}"))?;
    if file.version != VECTOR_VERSION {
        return Err(format!(
            "capability vector version {} is not supported by this build (expects {VECTOR_VERSION})",
            file.version
        ));
    }
    if file.capabilities.is_empty() {
        return Err("capability vector contains no capabilities".into());
    }
    Ok(DeviceProfile {
        model: file.model,
        capabilities: file.capabilities,
    })
}

/// The HT08 / MeloBuds Pro profile, compiled from the shared conformance vector.
pub fn ht08_profile() -> DeviceProfile {
    const VECTOR: &str = include_str!("../../../../conformance/capabilities_ht08.json");
    parse_vector(VECTOR).expect("the committed HT08 capability vector must parse")
}

impl DeviceProfile {
    /// Look up one capability by its stable id (e.g. `ancOff`, `systemEq`).
    pub fn get(&self, id: &str) -> Option<&CapabilityTruth> {
        self.capabilities.get(id)
    }
}

/* ------------------------------------------------------------------ */
/* Derivation rules — mirror src/lib/qcy/device/capabilities.ts        */
/* ------------------------------------------------------------------ */

/// Show a capability unless both hardware and protocol say it does not exist.
pub fn is_shown(cap: &CapabilityTruth) -> bool {
    !(cap.hardware == EvidenceTruth::Unsupported && cap.protocol == EvidenceTruth::Unsupported)
}

/// Implemented in this build (not mock-only, not pending).
pub fn is_implemented(cap: &CapabilityTruth) -> bool {
    cap.implementation == ImplTruth::Implemented
}

/// A supported write the app implements.
pub fn is_writable(cap: &CapabilityTruth) -> bool {
    is_implemented(cap) && cap.write == WriteReadiness::Writable
}

/// An experimental write the app implements (needs the session opt-in).
pub fn is_experimental_write(cap: &CapabilityTruth) -> bool {
    is_implemented(cap) && cap.write == WriteReadiness::Experimental
}

/// The UI may offer interaction: implemented and either writable or experimental.
pub fn can_interact(cap: &CapabilityTruth) -> bool {
    is_implemented(cap)
        && matches!(
            cap.write,
            WriteReadiness::Writable | WriteReadiness::Experimental
        )
}

/// Summary tone for UI chips; mirrors the TypeScript `CapabilitySummary.tone`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryTone {
    Supported,
    Experimental,
    Neutral,
    Unknown,
    Research,
    Danger,
}

/// Honest one-line summary for chips/UI, derived from the four truths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilitySummary {
    pub label: &'static str,
    pub tone: SummaryTone,
}

/// Mirrors `summarizeCapability` in the TypeScript model, branch for branch.
pub fn summarize_capability(cap: &CapabilityTruth) -> CapabilitySummary {
    if cap.write == WriteReadiness::Forbidden {
        return CapabilitySummary {
            label: "Forbidden",
            tone: SummaryTone::Danger,
        };
    }
    if cap.implementation == ImplTruth::Implemented {
        return match cap.write {
            WriteReadiness::Experimental => CapabilitySummary {
                label: "Experimental",
                tone: SummaryTone::Experimental,
            },
            WriteReadiness::Writable => CapabilitySummary {
                label: "Supported",
                tone: SummaryTone::Supported,
            },
            _ => CapabilitySummary {
                label: "Read-only",
                tone: SummaryTone::Supported,
            },
        };
    }
    if cap.implementation == ImplTruth::MockOnly {
        return CapabilitySummary {
            label: "Mock only",
            tone: SummaryTone::Neutral,
        };
    }
    // not-implemented below here
    if cap.protocol == EvidenceTruth::Supported {
        return CapabilitySummary {
            label: "Protocol known \u{00b7} app pending",
            tone: SummaryTone::Neutral,
        };
    }
    if cap.hardware == EvidenceTruth::Unsupported && cap.protocol == EvidenceTruth::Unsupported {
        return CapabilitySummary {
            label: "Unsupported",
            tone: SummaryTone::Neutral,
        };
    }
    if cap.hardware == EvidenceTruth::Supported {
        return CapabilitySummary {
            label: "Needs protocol research",
            tone: SummaryTone::Research,
        };
    }
    CapabilitySummary {
        label: "Unknown",
        tone: SummaryTone::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cap(
        hardware: EvidenceTruth,
        protocol: EvidenceTruth,
        implementation: ImplTruth,
        write: WriteReadiness,
    ) -> CapabilityTruth {
        CapabilityTruth {
            hardware,
            protocol,
            implementation,
            write,
            note: None,
            opcode: None,
        }
    }

    #[test]
    fn derivation_rules_mirror_the_typescript_model() {
        let writable = cap(
            EvidenceTruth::Supported,
            EvidenceTruth::Supported,
            ImplTruth::Implemented,
            WriteReadiness::Writable,
        );
        assert!(is_shown(&writable));
        assert!(is_implemented(&writable));
        assert!(is_writable(&writable));
        assert!(can_interact(&writable));

        let experimental = cap(
            EvidenceTruth::Supported,
            EvidenceTruth::Supported,
            ImplTruth::Implemented,
            WriteReadiness::Experimental,
        );
        assert!(is_experimental_write(&experimental));
        assert!(can_interact(&experimental));
        assert!(!is_writable(&experimental));

        let read_only = cap(
            EvidenceTruth::Supported,
            EvidenceTruth::Supported,
            ImplTruth::Implemented,
            WriteReadiness::ReadOnly,
        );
        assert!(!can_interact(&read_only));
        assert!(!is_writable(&read_only));

        // A writable readiness without implementation is still not interactable.
        let pending = cap(
            EvidenceTruth::Supported,
            EvidenceTruth::Supported,
            ImplTruth::NotImplemented,
            WriteReadiness::Writable,
        );
        assert!(!can_interact(&pending));
        assert!(!is_writable(&pending));

        let hidden = cap(
            EvidenceTruth::Unsupported,
            EvidenceTruth::Unsupported,
            ImplTruth::NotImplemented,
            WriteReadiness::ReadOnly,
        );
        assert!(!is_shown(&hidden));
    }

    #[test]
    fn summary_branches_match_the_typescript_labels() {
        use EvidenceTruth::*;
        use ImplTruth::*;
        use WriteReadiness::*;
        let cases = [
            (
                cap(Supported, Supported, Implemented, Forbidden),
                "Forbidden",
                SummaryTone::Danger,
            ),
            (
                cap(Supported, Supported, Implemented, Experimental),
                "Experimental",
                SummaryTone::Experimental,
            ),
            (
                cap(Supported, Supported, Implemented, Writable),
                "Supported",
                SummaryTone::Supported,
            ),
            (
                cap(Supported, Supported, Implemented, ReadOnly),
                "Read-only",
                SummaryTone::Supported,
            ),
            (
                cap(Supported, Supported, MockOnly, ReadOnly),
                "Mock only",
                SummaryTone::Neutral,
            ),
            (
                cap(Supported, Supported, NotImplemented, Writable),
                "Protocol known \u{00b7} app pending",
                SummaryTone::Neutral,
            ),
            (
                cap(Unsupported, Unsupported, NotImplemented, ReadOnly),
                "Unsupported",
                SummaryTone::Neutral,
            ),
            (
                cap(Supported, Unknown, NotImplemented, ReadOnly),
                "Needs protocol research",
                SummaryTone::Research,
            ),
            (
                cap(Unknown, Unknown, NotImplemented, ReadOnly),
                "Unknown",
                SummaryTone::Unknown,
            ),
        ];
        for (cap, label, tone) in cases {
            let s = summarize_capability(&cap);
            assert_eq!(s.label, label, "label mismatch for {cap:?}");
            assert_eq!(s.tone, tone, "tone mismatch for {cap:?}");
        }
    }

    #[test]
    fn rejects_unknown_vector_versions() {
        let json = r#"{"version": 99, "model": "HT08", "source": "x", "capabilities": {"a": {"hardware": "unknown", "protocol": "unknown", "implementation": "not-implemented", "write": "read-only"}}}"#;
        let err = parse_vector(json).unwrap_err();
        assert!(err.contains("not supported"), "{err}");
    }

    #[test]
    fn rejects_empty_capability_maps() {
        let json = r#"{"version": 1, "model": "HT08", "source": "x", "capabilities": {}}"#;
        assert!(parse_vector(json).is_err());
    }
}
