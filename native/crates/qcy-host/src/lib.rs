//! Linux host services for 521C (issue #13).
//!
//! This crate is the host-side counterpart to [`qcy_transport`] (device I/O). It owns
//! functionality that lives on the Linux machine, NOT in the earbuds:
//!
//! * [`mpris`] — media discovery/state/control over the MPRIS D-Bus interface.
//! * [`codec`] — codec/sample-rate/profile state from the host audio graph, reported as
//!   unknown when it cannot be sourced reliably.
//! * [`game_mode`] — Auto Game Mode: an event-driven signal + debounce + keyword
//!   allowlist. Never a busy polling loop.
//! * [`system_eq`] — System EQ through an explicit PipeWire-compatible host path with
//!   clear create/remove lifecycle and no system-wide configuration.
//!
//! Host-only state is deliberately separate from the QCY vendor protocol and from the
//! capability/truth model's device dimensions: none of this is ever written to the buds,
//! and none of it is presented as earbud DSP support.
//!
//! Every external boundary (D-Bus, filesystem, audio graph) is isolated behind a small
//! trait so the logic is unit-tested against fakes when no live service is available.

pub mod codec;
pub mod game_mode;
pub mod mpris;
pub mod system_eq;

/// Structured host-service errors. Missing services are a normal, expected state on a
/// host without the relevant daemon and must be handled gracefully, not treated as a
/// crash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    /// The required service (D-Bus session bus, MPRIS player, PipeWire) is not present.
    ServiceUnavailable(String),
    /// The requested player/target was not found.
    NotFound(String),
    /// The operation is not supported by the backend or this build.
    Unsupported(String),
    /// A backend call failed in a way that does not map to a more specific variant.
    Backend(String),
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostError::ServiceUnavailable(what) => write!(f, "service unavailable: {what}"),
            HostError::NotFound(what) => write!(f, "not found: {what}"),
            HostError::Unsupported(what) => write!(f, "unsupported: {what}"),
            HostError::Backend(msg) => write!(f, "host backend error: {msg}"),
        }
    }
}

impl std::error::Error for HostError {}

/// Aggregate read-only snapshot of host services, suitable for surfacing to a UI/CLI
/// without leaking D-Bus or audio-graph internals.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HostSnapshot {
    pub media: mpris::MediaStatus,
    pub codec: codec::CodecInfo,
    pub game_mode_active: bool,
    pub system_eq: system_eq::SystemEqStatus,
}
