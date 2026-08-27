//! Transport abstraction for QCY BLE device I/O (issue #7).
//!
//! This crate sits above [`qcy_protocol`] (framing/codecs) and below the CLI/GUI. It
//! defines a single [`Transport`] trait with three backends:
//!
//!   * [`mock::MockTransport`] — deterministic, hardware-free, used by tests and dev.
//!   * [`bluez::BlueZTransport`] — talks to the system BlueZ stack over D-Bus GATT.
//!   * [`rfcomm::RfcommTransport`] — SPP/RFCOMM byte stream (issue #50): the control
//!     path BlueZ actually exposes for QCY dual-mode earbuds on Linux, carrying the
//!     same `0xFF`-framed protocol as the GATT path.
//!
//! All outbound operations are checked against the central [`policy::WritePolicy`]
//! (the Rust mirror of issue #1's authorization model) before reaching the wire. The
//! BlueZ backend is event-driven and never requires root or a reconfigured daemon;
//! the RFCOMM backend opens a raw `AF_BLUETOOTH` socket, also without root.

pub mod bluez;
pub mod mock;
pub mod policy;
pub mod rfcomm;

pub use policy::{Denial, WritePolicy};

/// Structured transport errors (issue #7): adapter-off, out-of-range, permission,
/// timeout, disconnect and policy denial are all distinguishable by callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    AdapterOff,
    DeviceOutOfRange,
    PermissionDenied,
    Timeout,
    Disconnected,
    NotFound(String),
    /// Rejected by the central write-authorization policy.
    Denied(Denial),
    InvalidArgument(String),
    /// Backend/D-Bus failure that does not map to a more specific variant.
    Bus(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::AdapterOff => write!(f, "Bluetooth adapter is off or missing"),
            TransportError::DeviceOutOfRange => write!(f, "device out of range"),
            TransportError::PermissionDenied => write!(f, "permission denied"),
            TransportError::Timeout => write!(f, "operation timed out"),
            TransportError::Disconnected => write!(f, "device disconnected"),
            TransportError::NotFound(what) => write!(f, "not found: {what}"),
            TransportError::Denied(d) => write!(f, "write denied: {d}"),
            TransportError::InvalidArgument(msg) => write!(f, "invalid argument: {msg}"),
            TransportError::Bus(msg) => write!(f, "transport error: {msg}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// A candidate device surfaced by discovery. `model_known` is false when the model is
/// not proven from advertisement/name evidence; such devices stay read-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredDevice {
    pub address: String,
    pub name: String,
    pub rssi: Option<i16>,
    pub model_known: bool,
}

/// Blocking, per-connection device transport. Implementations must be safe to drive
/// from a single thread; the BlueZ backend uses the blocking D-Bus API and is
/// event-driven (no polling loop).
pub trait Transport {
    /// Discover candidate QCY devices. Preserves unknown-device status when the model
    /// is not proven.
    fn scan(&mut self) -> Result<Vec<DiscoveredDevice>, TransportError>;
    /// Devices that are already connected at the host level (e.g. earbuds the
    /// user connected for audio before opening the application), so the app can
    /// attach to them without a user-initiated scan. Model truth is preserved:
    /// a listed device is writable only when its model is proven or previously
    /// confirmed. Default: none (mock/fake transports have no host connection
    /// state); transports with real host state override this.
    fn connected_devices(&mut self) -> Result<Vec<DiscoveredDevice>, TransportError> {
        Ok(Vec::new())
    }
    /// Connect to the device with the given address and resolve required characteristics.
    fn connect(&mut self, address: &str) -> Result<(), TransportError>;
    fn disconnect(&mut self) -> Result<(), TransportError>;
    /// Read an allowlisted characteristic.
    fn read(&mut self, char_uuid: &str) -> Result<Vec<u8>, TransportError>;
    /// Framed write to the command characteristic. Policy-checked.
    fn write(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    /// Unframed write to a specific allowlisted characteristic. Policy-checked.
    fn write_direct(&mut self, char_uuid: &str, bytes: &[u8]) -> Result<(), TransportError>;
    /// Subscribe to notifications on a characteristic (event-driven; no polling).
    fn subscribe(&mut self, char_uuid: &str) -> Result<(), TransportError>;
    /// Session-scoped opt-in for `write-experimental` opcodes (never persisted;
    /// resets when the transport is recreated). Without it, the central policy
    /// denies experimental writes.
    fn set_experimental_opt_in(&mut self, on: bool);
    /// Explicit user attestation that the connected device's model is known
    /// (e.g. a renamed HT08 whose advertised name no longer proves the model).
    /// Lifts the read-only state for the current connection only; persistence of
    /// the attestation is owned by the application layer. Must be a no-op when
    /// not connected. Destructive opcodes stay forbidden regardless.
    fn attest_model_known(&mut self);
}

/// Normalize a Bluetooth address for comparison/persistence: uppercase, `-` → `:`.
pub fn normalize_address(address: &str) -> String {
    address.trim().replace('-', ":").to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::normalize_address;

    #[test]
    fn normalize_address_uppercases_and_unifies_separators() {
        assert_eq!(normalize_address("84:ac:60:62:69:da"), "84:AC:60:62:69:DA");
        assert_eq!(normalize_address("84-ac-60-62-69-da"), "84:AC:60:62:69:DA");
        assert_eq!(
            normalize_address("  84:AC:60:62:69:DA  "),
            "84:AC:60:62:69:DA"
        );
    }
}
