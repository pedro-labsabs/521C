//! 521C application core (issue #8).
//!
//! This crate is the orchestration layer between the UI (Slint GUI or CLI) and the
//! lower layers:
//!
//! ```text
//! UI -> qcy-app (state/actions/config) -> device profile + protocol codec -> authorized transport
//!                                    \-> Linux host services (qcy-host)
//! ```
//!
//! It owns:
//!
//! * [`config`] — versioned, validated XDG persistence sharing the exact JSON
//!   contract of the browser schema (issue #11), so exported configs move between
//!   browser and desktop without mutation.
//! * [`core`] — the typed application state machine driving a [`qcy_transport::Transport`]
//!   and [`qcy_host`] services. Raw GATT bytes stay below this boundary; UIs consume
//!   typed snapshots and send typed commands. Write authorization is never
//!   reimplemented here — every write still converges on the transport's central
//!   [`qcy_transport::policy::WritePolicy`].

pub mod config;
pub mod core;
