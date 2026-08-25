//! Central write-authorization policy (Rust mirror of issue #1).
//!
//! The sets below are derived from the canonical evidence ledger
//! (`src/lib/qcy/protocol/evidence.ts` in the web tree). An opcode is writable only if
//! the ledger records it as `write-supported`; `write-experimental` opcodes additionally
//! require a session opt-in. Destructive opcodes are never authorized for automation.

use std::collections::HashSet;

/// Full 128-bit UUIDs for the direct-write characteristics (from `uuids.ts`).
pub const CHAR_EQ_DIRECT: &str = "0000000b-0000-1000-8000-00805f9b34fb";
pub const CHAR_KEY_FUNCTION_V2: &str = "0000000d-0000-1000-8000-00805f9b34fb";
/// Main QCY GATT service UUID.
pub const SERVICE_MAIN: &str = "0000a001-0000-1000-8000-00805f9b34fb";
/// Framed command write characteristic UUID.
pub const CHAR_COMMAND_WRITE: &str = "00001001-0000-1000-8000-00805f9b34fb";
/// Settings notification characteristic UUID.
pub const CHAR_SETTINGS_NOTIFY: &str = "00001002-0000-1000-8000-00805f9b34fb";

/// Opcodes the ledger records as `write-supported` for HT08.
const SUPPORTED_OPCODES: &[u8] = &[
    0x05, // LightFlash
    0x06, // InEarDetection
    0x09, // LowLatency
    0x0C, // NoiseCancelMode
    0x10, // SleepMode
    0x16, // SoundBalance
    0x17, // AncSetting
    0x22, // EqParamsV2
    0x2C, // WearingDetection
    0x3D, // TonePlay
];

/// Opcodes the ledger records as `write-experimental` (need a session opt-in).
const EXPERIMENTAL_OPCODES: &[u8] = &[
    0x23, // Ldac
    0x2D, // SpatialAudio
    0x32, // EnvAdaptation
];

/// Destructive opcodes — never authorized for unattended automation.
const DESTRUCTIVE_OPCODES: &[u8] = &[0x01, 0x02, 0x03];

/// Why a write was denied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Denial {
    ReadOnlyDevice,
    DestructiveOpcode(u8),
    OpcodeNotAuthorized(u8),
    ExperimentalWithoutOptIn(u8),
    CharacteristicNotAuthorized(String),
    MalformedFrame,
}

impl std::fmt::Display for Denial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Denial::ReadOnlyDevice => write!(f, "device is read-only"),
            Denial::DestructiveOpcode(op) => {
                write!(f, "destructive opcode 0x{op:02X} is never sent")
            }
            Denial::OpcodeNotAuthorized(op) => {
                write!(f, "opcode 0x{op:02X} is not authorized for writes")
            }
            Denial::ExperimentalWithoutOptIn(op) => {
                write!(
                    f,
                    "opcode 0x{op:02X} is experimental and needs a session opt-in"
                )
            }
            Denial::CharacteristicNotAuthorized(uuid) => {
                write!(
                    f,
                    "characteristic {uuid} is not authorized for direct writes"
                )
            }
            Denial::MalformedFrame => write!(f, "malformed frame"),
        }
    }
}

/// Per-profile write authorization surface.
#[derive(Debug, Clone)]
pub struct WritePolicy {
    pub supported_opcodes: HashSet<u8>,
    pub experimental_opcodes: HashSet<u8>,
    pub direct_chars: HashSet<String>,
    /// Unknown/generic devices are read-only by default.
    pub read_only: bool,
}

impl WritePolicy {
    /// HT08 trusted write surface, derived from the evidence ledger.
    pub fn ht08() -> Self {
        Self {
            supported_opcodes: SUPPORTED_OPCODES.iter().copied().collect(),
            experimental_opcodes: EXPERIMENTAL_OPCODES.iter().copied().collect(),
            direct_chars: [CHAR_EQ_DIRECT.to_string(), CHAR_KEY_FUNCTION_V2.to_string()]
                .into_iter()
                .collect(),
            read_only: false,
        }
    }

    /// Read-only policy for unknown/generic devices.
    pub fn read_only() -> Self {
        Self {
            supported_opcodes: HashSet::new(),
            experimental_opcodes: HashSet::new(),
            direct_chars: HashSet::new(),
            read_only: true,
        }
    }

    pub fn is_destructive(op: u8) -> bool {
        DESTRUCTIVE_OPCODES.contains(&op)
    }

    /// Extract the opcode from a framed command (`SOF LEN CMD ...`). Returns None when
    /// the frame is too short to carry an opcode.
    fn frame_opcode(bytes: &[u8]) -> Option<u8> {
        // Minimal framing check: SOF, length, then at least one command byte.
        if bytes.len() < 3 || bytes[0] != qcy_protocol::SOF {
            return None;
        }
        Some(bytes[2])
    }

    /// Authorize a framed write to the command characteristic.
    pub fn authorize_frame(&self, bytes: &[u8], experimental_opt_in: bool) -> Result<(), Denial> {
        if self.read_only {
            return Err(Denial::ReadOnlyDevice);
        }
        let op = Self::frame_opcode(bytes).ok_or(Denial::MalformedFrame)?;
        if Self::is_destructive(op) {
            return Err(Denial::DestructiveOpcode(op));
        }
        if self.supported_opcodes.contains(&op) {
            return Ok(());
        }
        if self.experimental_opcodes.contains(&op) {
            if experimental_opt_in {
                return Ok(());
            }
            return Err(Denial::ExperimentalWithoutOptIn(op));
        }
        Err(Denial::OpcodeNotAuthorized(op))
    }

    /// Authorize an unframed direct write to an allowlisted characteristic.
    pub fn authorize_direct(
        &self,
        char_uuid: &str,
        _bytes: &[u8],
        _experimental_opt_in: bool,
    ) -> Result<(), Denial> {
        if self.read_only {
            return Err(Denial::ReadOnlyDevice);
        }
        let normalized = char_uuid.to_ascii_lowercase();
        if self
            .direct_chars
            .iter()
            .any(|c| c.to_ascii_lowercase() == normalized)
        {
            Ok(())
        } else {
            Err(Denial::CharacteristicNotAuthorized(char_uuid.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qcy_protocol::packet::encode_command;

    #[test]
    fn supported_opcode_is_authorized() {
        let p = WritePolicy::ht08();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        assert!(p.authorize_frame(&frame, false).is_ok());
    }

    #[test]
    fn experimental_opcode_requires_opt_in() {
        let p = WritePolicy::ht08();
        let frame = encode_command(0x32, &[0x01]).unwrap();
        assert!(matches!(
            p.authorize_frame(&frame, false),
            Err(Denial::ExperimentalWithoutOptIn(0x32))
        ));
        assert!(p.authorize_frame(&frame, true).is_ok());
    }

    #[test]
    fn destructive_opcodes_are_never_authorized() {
        let p = WritePolicy::ht08();
        for op in [0x01u8, 0x02, 0x03] {
            let frame = encode_command(op, &[]).unwrap();
            assert!(matches!(
                p.authorize_frame(&frame, true),
                Err(Denial::DestructiveOpcode(_))
            ));
        }
    }

    #[test]
    fn catalog_only_opcode_is_not_authorized() {
        let p = WritePolicy::ht08();
        let frame = encode_command(0x18, &[0x41]).unwrap(); // RenameDevice: catalog-only
        assert!(matches!(
            p.authorize_frame(&frame, false),
            Err(Denial::OpcodeNotAuthorized(0x18))
        ));
    }

    #[test]
    fn read_only_device_denies_everything() {
        let p = WritePolicy::read_only();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        assert!(matches!(
            p.authorize_frame(&frame, false),
            Err(Denial::ReadOnlyDevice)
        ));
        assert!(matches!(
            p.authorize_direct(CHAR_KEY_FUNCTION_V2, &[], false),
            Err(Denial::ReadOnlyDevice)
        ));
    }

    #[test]
    fn direct_write_requires_allowlisted_characteristic() {
        let p = WritePolicy::ht08();
        assert!(p
            .authorize_direct(CHAR_KEY_FUNCTION_V2, &[1, 2], false)
            .is_ok());
        assert!(matches!(
            p.authorize_direct("0000dead-0000-1000-8000-00805f9b34fb", &[1], false),
            Err(Denial::CharacteristicNotAuthorized(_))
        ));
    }
}
