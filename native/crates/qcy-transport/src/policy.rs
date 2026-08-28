//! Central write-authorization policy (Rust mirror of issue #1).
//!
//! The sets below are derived from the canonical evidence ledger
//! (`src/lib/qcy/protocol/evidence.ts` in the web tree) and are pinned against
//! the shared conformance corpus (`conformance/protocol_vectors.json`,
//! section `writePolicy.ht08`) by `tests/conformance_write_policy.rs` and the
//! TS suite — the #53 demotion of 0x0C drifted here unnoticed once (audit #59);
//! the corpus pin makes any future drift fail CI on both sides.
//!
//! An opcode is writable only if the ledger records it as `write-supported`;
//! `write-experimental` opcodes additionally require a session opt-in (the
//! native policy has no pure-disable exception — see docs/SECURITY_MODEL.md).
//! Destructive opcodes are never authorized for automation.

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
    0x10, // SleepMode
    0x16, // SoundBalance
    0x17, // AncSetting
    0x22, // EqParamsV2
    0x2C, // WearingDetection
    0x3D, // TonePlay
];

/// Opcodes the ledger records as `write-experimental` (need a session opt-in).
const EXPERIMENTAL_OPCODES: &[u8] = &[
    0x0C, // NoiseCancelMode — falsified on live HT08 (#52/#53): writes ignored,
          // no ACK; ANC state uses the validated 0x17 scene table instead.
    0x23, // Ldac
    0x2D, // SpatialAudio
    0x32, // EnvAdaptation
];

/// Destructive opcodes — never authorized for unattended automation.
const DESTRUCTIVE_OPCODES: &[u8] = &[0x01, 0x02, 0x03];

/// RequestData opcode: a read-back request, not a state mutation. Mirrors the
/// TypeScript policy (`src/lib/qcy/policy.ts`), which authorizes it even for
/// read-only devices so status/identification can be read. The SPP/RFCOMM
/// transport (issue #50) depends on this: stream reads are `0xFE` frames.
const REQUEST_DATA_OPCODE: u8 = 0xFE;

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

    /// Authorize a framed write to the command characteristic.
    ///
    /// The whole frame is decoded first and **every** command block is
    /// authorized, mirroring the TypeScript policy (`src/lib/qcy/policy.ts`)
    /// and `docs/SECURITY_MODEL.md` ("for every command block in a frame").
    /// A supported first block can never smuggle a destructive, catalog-only
    /// or experimental block past the policy, and an undecodable frame is
    /// denied before it can reach the wire.
    pub fn authorize_frame(&self, bytes: &[u8], experimental_opt_in: bool) -> Result<(), Denial> {
        // Decode first: an undecodable frame is refused before any profile
        // judgment, mirroring the TypeScript policy (`undecodable-frame`).
        let packet =
            qcy_protocol::packet::decode_packet(bytes).map_err(|_| Denial::MalformedFrame)?;
        for block in &packet.blocks {
            let op = block.cmd;
            if Self::is_destructive(op) {
                return Err(Denial::DestructiveOpcode(op));
            }
            // RequestData (0xFE) is a read-back request, not a state mutation.
            // It is allowed even for read-only profiles so status/identification
            // can be read — same rule as the TypeScript policy.
            if op == REQUEST_DATA_OPCODE {
                continue;
            }
            // Unknown/generic devices are read-only by default: no state-changing
            // writes past this point.
            if self.read_only {
                return Err(Denial::ReadOnlyDevice);
            }
            if self.supported_opcodes.contains(&op) {
                continue;
            }
            if self.experimental_opcodes.contains(&op) {
                if experimental_opt_in {
                    continue;
                }
                return Err(Denial::ExperimentalWithoutOptIn(op));
            }
            return Err(Denial::OpcodeNotAuthorized(op));
        }
        Ok(())
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
    use qcy_protocol::packet::{encode_blocks, encode_command, CommandBlock};

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
    fn multi_block_frame_with_only_supported_blocks_is_authorized() {
        let p = WritePolicy::ht08();
        let frame = encode_blocks(&[
            CommandBlock {
                cmd: 0x09,
                params: vec![0x01],
            },
            CommandBlock {
                cmd: 0x10,
                params: vec![0x01],
            },
        ])
        .unwrap();
        assert!(p.authorize_frame(&frame, false).is_ok());
    }

    #[test]
    fn destructive_opcode_hidden_in_a_multi_block_frame_is_denied() {
        let p = WritePolicy::ht08();
        for op in [0x01u8, 0x02, 0x03] {
            let frame = encode_blocks(&[
                CommandBlock {
                    cmd: 0x09,
                    params: vec![0x01],
                },
                CommandBlock {
                    cmd: op,
                    params: vec![],
                },
            ])
            .unwrap();
            assert!(
                matches!(
                    p.authorize_frame(&frame, true),
                    Err(Denial::DestructiveOpcode(hidden)) if hidden == op
                ),
                "destructive 0x{op:02X} behind a supported block must be denied even with opt-in"
            );
        }
    }

    #[test]
    fn experimental_opcode_hidden_in_a_multi_block_frame_needs_opt_in() {
        let p = WritePolicy::ht08();
        let frame = encode_blocks(&[
            CommandBlock {
                cmd: 0x09,
                params: vec![0x01],
            },
            CommandBlock {
                cmd: 0x23,
                params: vec![0x01],
            },
        ])
        .unwrap();
        assert!(matches!(
            p.authorize_frame(&frame, false),
            Err(Denial::ExperimentalWithoutOptIn(0x23))
        ));
        assert!(p.authorize_frame(&frame, true).is_ok());
    }

    #[test]
    fn catalog_only_opcode_hidden_in_a_multi_block_frame_is_denied() {
        let p = WritePolicy::ht08();
        let frame = encode_blocks(&[
            CommandBlock {
                cmd: 0x09,
                params: vec![0x01],
            },
            CommandBlock {
                cmd: 0x18,
                params: vec![0x41],
            },
        ])
        .unwrap();
        assert!(matches!(
            p.authorize_frame(&frame, false),
            Err(Denial::OpcodeNotAuthorized(0x18))
        ));
    }

    #[test]
    fn malformed_frames_are_denied_before_any_block_is_considered() {
        let p = WritePolicy::ht08();
        // Bogus declared length (longer than the actual body).
        assert!(matches!(
            p.authorize_frame(&[0xFF, 0x40, 0x09], false),
            Err(Denial::MalformedFrame)
        ));
        // Bad SOF.
        assert!(matches!(
            p.authorize_frame(&[0x00, 0x01, 0x09], false),
            Err(Denial::MalformedFrame)
        ));
        // Truncated header.
        assert!(matches!(
            p.authorize_frame(&[0xFF], false),
            Err(Denial::MalformedFrame)
        ));
        // Declared a block whose params are truncated.
        assert!(matches!(
            p.authorize_frame(&[0xFF, 0x03, 0x09, 0x02], false),
            Err(Denial::MalformedFrame)
        ));
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

    #[test]
    fn request_data_is_authorized_even_on_read_only_devices() {
        // Mirrors the TypeScript policy: RequestData (0xFE) is a read-back
        // request, not a state mutation. The SPP transport (issue #50) sends
        // these frames to read battery/version from read-only devices too.
        let p = WritePolicy::read_only();
        let frame = encode_command(0xFE, &[0x2F]).unwrap();
        assert!(p.authorize_frame(&frame, false).is_ok());
        let ht08 = WritePolicy::ht08();
        assert!(ht08.authorize_frame(&frame, false).is_ok());
    }

    #[test]
    fn request_data_cannot_smuggle_a_state_change_past_read_only() {
        let p = WritePolicy::read_only();
        let frame = encode_blocks(&[
            CommandBlock {
                cmd: 0xFE,
                params: vec![0x2F],
            },
            CommandBlock {
                cmd: 0x09,
                params: vec![0x01],
            },
        ])
        .unwrap();
        assert!(matches!(
            p.authorize_frame(&frame, false),
            Err(Denial::ReadOnlyDevice)
        ));
    }

    #[test]
    fn destructive_opcode_beats_read_only_verdict() {
        // Destructive is checked before the read-only judgment, mirroring the
        // TypeScript ordering: the refusal reason must never understate the
        // danger of a reset/factory-reset frame.
        let p = WritePolicy::read_only();
        let frame = encode_command(0x03, &[]).unwrap();
        assert!(matches!(
            p.authorize_frame(&frame, false),
            Err(Denial::DestructiveOpcode(0x03))
        ));
    }

    #[test]
    fn malformed_frame_is_denied_even_on_read_only_devices() {
        let p = WritePolicy::read_only();
        assert!(matches!(
            p.authorize_frame(&[0xFF, 0x40, 0x09], false),
            Err(Denial::MalformedFrame)
        ));
    }
}
