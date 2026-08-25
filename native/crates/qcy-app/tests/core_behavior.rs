//! Behavioral tests for the typed application core (issue #8).
//!
//! These tests drive [`qcy_app::core::AppCore`] through a recording transport that
//! enforces the same invariants as the real transports: unknown models stay
//! read-only, and every framed write passes through the central `WritePolicy`.
//! They pin the safety contract the desktop UI depends on:
//!
//! * Find Earbuds is refused without the interactive preflight flag;
//! * policy denials surface as `AppEvent::Denied`, never as silent failure;
//! * connect enriches the snapshot only from discovery evidence.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use qcy_app::core::{AppCommand, AppCore, AppEvent, HostServices, SimpleNoise};
use qcy_protocol::packet::decode_packet;
use qcy_transport::{DiscoveredDevice, Transport, TransportError, WritePolicy};

const ADDR: &str = "AA:BB:CC:DD:EE:FF";

#[derive(Default)]
struct Shared {
    writes: Vec<Vec<u8>>,
    opt_in: bool,
}

/// Recording transport. Mirrors the real transports' safety shape: the connected
/// device is writable only when its model is proven, and every frame is authorized
/// by the central policy before it is recorded.
struct TestTransport {
    policy: WritePolicy,
    model_known: bool,
    connected: bool,
    connected_model_known: bool,
    shared: Arc<Mutex<Shared>>,
}

impl TestTransport {
    fn new(model_known: bool) -> (Self, Arc<Mutex<Shared>>) {
        let shared = Arc::default();
        (
            Self {
                policy: WritePolicy::ht08(),
                model_known,
                connected: false,
                connected_model_known: false,
                shared: Arc::clone(&shared),
            },
            shared,
        )
    }
}

impl Transport for TestTransport {
    fn scan(&mut self) -> Result<Vec<DiscoveredDevice>, TransportError> {
        Ok(vec![DiscoveredDevice {
            address: ADDR.into(),
            name: if self.model_known {
                "QCY MeloBuds Pro".into()
            } else {
                "QCY T20".into()
            },
            rssi: Some(-55),
            model_known: self.model_known,
        }])
    }

    fn connect(&mut self, _address: &str) -> Result<(), TransportError> {
        self.connected = true;
        self.connected_model_known = self.model_known;
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
        if uuid.contains("00000008") {
            Ok(vec![0x52, 0x50, 0x5E])
        } else if uuid.contains("00000007") {
            Ok(vec![1, 4, 2])
        } else {
            Err(TransportError::NotFound(char_uuid.into()))
        }
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::Disconnected);
        }
        if !self.connected_model_known {
            return Err(TransportError::Denied(
                qcy_transport::policy::Denial::ReadOnlyDevice,
            ));
        }
        let opt_in = self.shared.lock().expect("shared mutex").opt_in;
        self.policy
            .authorize_frame(bytes, opt_in)
            .map_err(TransportError::Denied)?;
        self.shared
            .lock()
            .expect("shared mutex")
            .writes
            .push(bytes.to_vec());
        Ok(())
    }

    fn write_direct(&mut self, _char_uuid: &str, _bytes: &[u8]) -> Result<(), TransportError> {
        Ok(())
    }

    fn subscribe(&mut self, _char_uuid: &str) -> Result<(), TransportError> {
        Ok(())
    }

    fn set_experimental_opt_in(&mut self, on: bool) {
        self.shared.lock().expect("shared mutex").opt_in = on;
    }
}

fn start(model_known: bool) -> (qcy_app::core::AppHandle, Arc<Mutex<Shared>>) {
    let (transport, shared) = TestTransport::new(model_known);
    (
        AppCore::start(Box::new(transport), HostServices::default()),
        shared,
    )
}

/// Collect events until one matches the predicate; fail with the seen events on timeout.
fn recv_until<F>(handle: &qcy_app::core::AppHandle, mut matches: F) -> AppEvent
where
    F: FnMut(&AppEvent) -> bool,
{
    let mut seen = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if let Some(event) = handle.try_recv_event(Duration::from_millis(100)) {
            if matches(&event) {
                return event;
            }
            seen.push(event);
        }
    }
    panic!("timed out waiting for event; saw: {seen:?}")
}

fn scan_and_connect(handle: &qcy_app::core::AppHandle) {
    handle.send(AppCommand::Scan).unwrap();
    recv_until(handle, |e| matches!(e, AppEvent::Discovered(_)));
    handle.send(AppCommand::Connect(ADDR.into())).unwrap();
    recv_until(
        handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );
}

#[test]
fn connect_enriches_snapshot_only_from_discovery_evidence() {
    let (handle, _shared) = start(true);
    handle.send(AppCommand::Scan).unwrap();
    recv_until(&handle, |e| matches!(e, AppEvent::Discovered(_)));
    handle.send(AppCommand::Connect(ADDR.into())).unwrap();
    let event = recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );
    let AppEvent::StateChanged(snapshot) = event else {
        unreachable!()
    };
    assert_eq!(snapshot.name, "QCY MeloBuds Pro");
    assert_eq!(snapshot.address, ADDR);
    assert_eq!(snapshot.rssi, Some(-55));
    assert!(snapshot.model_known);
    let battery = snapshot.battery.expect("battery readout");
    assert_eq!(
        (battery.left, battery.right, battery.case),
        (0x52, 0x50, 0x5E)
    );
    assert_eq!(snapshot.firmware.as_deref(), Some("1.4.2"));
    handle.shutdown();
}

#[test]
fn find_chime_without_preflight_is_refused_before_any_write() {
    let (handle, shared) = start(true);
    scan_and_connect(&handle);
    handle
        .send(AppCommand::FindChime {
            led: true,
            chime: true,
            tone_id: 0x01,
            confirmed_not_worn: false,
        })
        .unwrap();
    recv_until(&handle, |e| matches!(e, AppEvent::Denied(_)));
    assert!(
        shared.lock().expect("shared mutex").writes.is_empty(),
        "no write may reach the transport without the preflight"
    );
    handle.shutdown();
}

#[test]
fn find_chime_with_preflight_writes_led_then_tone() {
    let (handle, shared) = start(true);
    scan_and_connect(&handle);
    handle
        .send(AppCommand::FindChime {
            led: true,
            chime: true,
            tone_id: 0x02,
            confirmed_not_worn: true,
        })
        .unwrap();
    recv_until(&handle, |e| matches!(e, AppEvent::Info(_)));
    let writes = shared.lock().expect("shared mutex").writes.clone();
    assert_eq!(writes.len(), 2);
    let led = decode_packet(&writes[0]).expect("led frame decodes");
    assert_eq!(led.blocks[0].cmd, qcy_protocol::Cmd::LightFlash as u8);
    let tone = decode_packet(&writes[1]).expect("tone frame decodes");
    assert_eq!(tone.blocks[0].cmd, qcy_protocol::Cmd::TonePlay as u8);
    assert_eq!(tone.blocks[0].params, vec![0x02]);
    handle.shutdown();
}

#[test]
fn supported_write_produces_an_authorized_frame() {
    let (handle, shared) = start(true);
    scan_and_connect(&handle);
    handle.send(AppCommand::SetGameMode(true)).unwrap();
    recv_until(&handle, |e| matches!(e, AppEvent::Info(_)));
    let writes = shared.lock().expect("shared mutex").writes.clone();
    assert_eq!(writes.len(), 1);
    let packet = decode_packet(&writes[0]).expect("frame decodes");
    assert_eq!(packet.blocks[0].cmd, qcy_protocol::Cmd::LowLatency as u8);
    assert_eq!(packet.blocks[0].params, vec![0x01]);
    handle.shutdown();
}

#[test]
fn noise_mode_maps_to_the_documented_opcode() {
    let (handle, shared) = start(true);
    scan_and_connect(&handle);
    handle
        .send(AppCommand::SetNoise(SimpleNoise::Transparency))
        .unwrap();
    recv_until(&handle, |e| matches!(e, AppEvent::Info(_)));
    let writes = shared.lock().expect("shared mutex").writes.clone();
    let packet = decode_packet(&writes[0]).expect("frame decodes");
    assert_eq!(
        packet.blocks[0].cmd,
        qcy_protocol::Cmd::NoiseCancelMode as u8
    );
    assert_eq!(packet.blocks[0].params, vec![0x03]);
    handle.shutdown();
}

#[test]
fn unknown_model_device_surfaces_denied_writes() {
    let (handle, shared) = start(false);
    scan_and_connect(&handle);
    handle.send(AppCommand::SetGameMode(true)).unwrap();
    let event = recv_until(&handle, |e| matches!(e, AppEvent::Denied(_)));
    let AppEvent::Denied(message) = event else {
        unreachable!()
    };
    assert!(
        message.to_lowercase().contains("read-only"),
        "denial should explain the read-only state, got: {message}"
    );
    assert!(
        shared.lock().expect("shared mutex").writes.is_empty(),
        "unknown devices must never receive writes"
    );
    handle.shutdown();
}

#[test]
fn experimental_opcode_requires_session_opt_in() {
    let (handle, shared) = start(true);
    scan_and_connect(&handle);

    // LDAC (0x23) is write-experimental: denied without the session opt-in.
    handle
        .send(AppCommand::SetExperimentalOptIn(false))
        .unwrap();
    recv_until(&handle, |e| matches!(e, AppEvent::Info(_)));
    // There is no public AppCommand for experimental opcodes (by design), so the
    // opt-in forwarding is verified through the transport's recorded flag.
    assert!(!shared.lock().expect("shared mutex").opt_in);

    handle.send(AppCommand::SetExperimentalOptIn(true)).unwrap();
    recv_until(&handle, |e| matches!(e, AppEvent::Info(_)));
    assert!(
        shared.lock().expect("shared mutex").opt_in,
        "opt-in must be forwarded to the transport policy"
    );
    handle.shutdown();
}

#[test]
fn disconnect_resets_the_snapshot() {
    let (handle, _shared) = start(true);
    scan_and_connect(&handle);
    handle.send(AppCommand::Disconnect).unwrap();
    let event = recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if !s.connected),
    );
    let AppEvent::StateChanged(snapshot) = event else {
        unreachable!()
    };
    assert!(snapshot.battery.is_none());
    assert!(snapshot.firmware.is_none());
    assert!(snapshot.address.is_empty());
    handle.shutdown();
}
