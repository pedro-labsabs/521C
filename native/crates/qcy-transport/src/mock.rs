//! Deterministic, hardware-free transport used by tests and `521cctl --mock`.

use std::collections::HashMap;

use qcy_protocol::packet::decode_packet;

use crate::policy::{WritePolicy, CHAR_COMMAND_WRITE};
use crate::{DiscoveredDevice, Transport, TransportError};

/// Address of the deterministic mock HT08 device.
pub const MOCK_HT08_ADDRESS: &str = "F8:5C:7D:12:08:08";

/// Mock HT08 device. Mirrors the web mock closely enough for CLI/dev work and tests.
#[derive(Debug)]
pub struct MockTransport {
    policy: WritePolicy,
    experimental_opt_in: bool,
    connected: bool,
    /// Model evidence for the connected address. Only the mock HT08 address is a
    /// proven model; any other address connects but stays read-only, mirroring the
    /// unknown-device invariant of the real transports.
    connected_model_known: bool,
    battery: Vec<u8>,
    firmware: Vec<u8>,
    direct: HashMap<String, Vec<u8>>,
    /// Every framed write that passed the policy, in order. Inspectable in tests.
    pub tx_log: Vec<Vec<u8>>,
}

impl MockTransport {
    pub fn new(policy: WritePolicy) -> Self {
        Self {
            policy,
            experimental_opt_in: false,
            connected: false,
            connected_model_known: false,
            battery: vec![0x52, 0x50, 0x5E], // 82 / 80 / 94
            firmware: vec![1, 4, 2, 1, 4, 2],
            direct: HashMap::new(),
            tx_log: Vec::new(),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }
}

impl Transport for MockTransport {
    fn scan(&mut self) -> Result<Vec<DiscoveredDevice>, TransportError> {
        Ok(vec![DiscoveredDevice {
            address: MOCK_HT08_ADDRESS.to_string(),
            name: "QCY MeloBuds Pro".to_string(),
            rssi: Some(-48),
            model_known: true,
        }])
    }

    fn connect(&mut self, address: &str) -> Result<(), TransportError> {
        self.connected = true;
        self.connected_model_known = address == MOCK_HT08_ADDRESS;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), TransportError> {
        self.connected = false;
        self.connected_model_known = false;
        Ok(())
    }

    fn read(&mut self, char_uuid: &str) -> Result<Vec<u8>, TransportError> {
        if !self.connected {
            return Err(TransportError::Disconnected);
        }
        let uuid = char_uuid.to_ascii_lowercase();
        if uuid.ends_with("0008-0000-1000-8000-00805f9b34fb") {
            Ok(self.battery.clone())
        } else if uuid.ends_with("0007-0000-1000-8000-00805f9b34fb") {
            Ok(self.firmware.clone())
        } else if let Some(v) = self.direct.get(&uuid) {
            Ok(v.clone())
        } else {
            Err(TransportError::NotFound(char_uuid.to_string()))
        }
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::Disconnected);
        }
        if !self.connected_model_known {
            return Err(TransportError::Denied(
                crate::policy::Denial::ReadOnlyDevice,
            ));
        }
        self.policy
            .authorize_frame(bytes, self.experimental_opt_in)
            .map_err(TransportError::Denied)?;
        // Validate framing before accepting the write.
        decode_packet(bytes).map_err(|e| TransportError::InvalidArgument(format!("{e:?}")))?;
        self.tx_log.push(bytes.to_vec());
        Ok(())
    }

    fn write_direct(&mut self, char_uuid: &str, bytes: &[u8]) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::Disconnected);
        }
        if !self.connected_model_known {
            return Err(TransportError::Denied(
                crate::policy::Denial::ReadOnlyDevice,
            ));
        }
        self.policy
            .authorize_direct(char_uuid, bytes, self.experimental_opt_in)
            .map_err(TransportError::Denied)?;
        self.direct
            .insert(char_uuid.to_ascii_lowercase(), bytes.to_vec());
        Ok(())
    }

    fn subscribe(&mut self, _char_uuid: &str) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::Disconnected);
        }
        Ok(())
    }

    fn set_experimental_opt_in(&mut self, on: bool) {
        self.experimental_opt_in = on;
    }

    fn attest_model_known(&mut self) {
        if self.connected {
            self.connected_model_known = true;
        }
    }
}

#[allow(dead_code)]
const _COMMAND_WRITE_REF: &str = CHAR_COMMAND_WRITE;

#[cfg(test)]
mod tests {
    use super::*;
    use qcy_protocol::packet::encode_command;

    fn connected_mock() -> MockTransport {
        let mut t = MockTransport::new(WritePolicy::ht08());
        t.connect("F8:5C:7D:12:08:08").unwrap();
        t
    }

    #[test]
    fn scan_reports_a_known_ht08() {
        let mut t = MockTransport::new(WritePolicy::ht08());
        let list = t.scan().unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].model_known);
    }

    #[test]
    fn write_requires_connection() {
        let mut t = MockTransport::new(WritePolicy::ht08());
        let frame = encode_command(0x09, &[0x01]).unwrap();
        assert_eq!(t.write(&frame), Err(TransportError::Disconnected));
    }

    #[test]
    fn supported_write_is_logged() {
        let mut t = connected_mock();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        t.write(&frame).unwrap();
        assert_eq!(t.tx_log.len(), 1);
    }

    #[test]
    fn destructive_write_is_denied() {
        let mut t = connected_mock();
        let frame = encode_command(0x01, &[]).unwrap();
        assert!(matches!(t.write(&frame), Err(TransportError::Denied(_))));
        assert!(t.tx_log.is_empty());
    }

    #[test]
    fn unknown_device_connects_but_stays_read_only() {
        let mut t = MockTransport::new(WritePolicy::ht08());
        t.connect("AA:BB:CC:DD:EE:FF").unwrap();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        match t.write(&frame) {
            Err(TransportError::Denied(crate::policy::Denial::ReadOnlyDevice)) => {}
            other => panic!("expected ReadOnlyDevice denial, got {other:?}"),
        }
        assert!(t.tx_log.is_empty());
        // Read-only still permits status reads.
        assert!(t.read("00000008-0000-1000-8000-00805f9b34fb").is_ok());
    }

    #[test]
    fn user_attestation_lifts_read_only_for_the_connection() {
        let mut t = MockTransport::new(WritePolicy::ht08());
        t.connect("AA:BB:CC:DD:EE:FF").unwrap();
        t.attest_model_known();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        t.write(&frame).unwrap();
        assert_eq!(t.tx_log.len(), 1);
        // Destructive opcodes stay forbidden even after attestation.
        let reset = encode_command(0x01, &[]).unwrap();
        assert!(matches!(t.write(&reset), Err(TransportError::Denied(_))));
    }

    #[test]
    fn attestation_is_cleared_by_disconnect_and_ignored_when_not_connected() {
        let mut t = MockTransport::new(WritePolicy::ht08());
        t.attest_model_known(); // no-op without a connection
        t.connect("AA:BB:CC:DD:EE:FF").unwrap();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        assert!(matches!(
            t.write(&frame),
            Err(TransportError::Denied(
                crate::policy::Denial::ReadOnlyDevice
            ))
        ));
        t.attest_model_known();
        t.write(&frame).unwrap();
        t.disconnect().unwrap();
        t.connect("AA:BB:CC:DD:EE:FF").unwrap();
        assert!(matches!(
            t.write(&frame),
            Err(TransportError::Denied(
                crate::policy::Denial::ReadOnlyDevice
            ))
        ));
    }

    #[test]
    fn disconnect_clears_model_evidence() {
        let mut t = connected_mock();
        t.disconnect().unwrap();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        assert_eq!(t.write(&frame), Err(TransportError::Disconnected));
    }

    #[test]
    fn read_returns_battery_and_firmware() {
        let mut t = connected_mock();
        let battery = t.read("00000008-0000-1000-8000-00805f9b34fb").unwrap();
        assert_eq!(battery, vec![0x52, 0x50, 0x5E]);
        let fw = t.read("00000007-0000-1000-8000-00805f9b34fb").unwrap();
        assert_eq!(fw, vec![1, 4, 2, 1, 4, 2]);
    }
}
