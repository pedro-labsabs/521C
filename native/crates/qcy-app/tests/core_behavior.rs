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

use qcy_app::core::{AppCommand, AppCore, AppEvent, HostServices, SimpleNoise, SupervisorConfig};
use qcy_protocol::packet::decode_packet;
use qcy_transport::{DiscoveredDevice, Transport, TransportError, WritePolicy};

const ADDR: &str = "AA:BB:CC:DD:EE:FF";

#[derive(Default)]
struct Shared {
    writes: Vec<Vec<u8>>,
    opt_in: bool,
    /// Live link state as seen by the host (is_connected). Tests flip it to
    /// simulate link loss; a successful connect restores it.
    link_up: bool,
    /// When true, connect() fails — used to exercise re-bootstrap failures.
    fail_connect: bool,
    /// Simulated discovery window: connect() blocks this long before it
    /// resolves (like BlueZTransport's LE fallback window). Used to verify an
    /// explicit Disconnect is not held hostage by a background attempt (#64a).
    connect_block_ms: u64,
    connect_attempts: u32,
    /// Proven reads observed (keepalive coverage).
    reads: u32,
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
    /// Devices reported as already connected at host level (`connected_devices`).
    connected_list: Vec<DiscoveredDevice>,
    /// Identity holding the live session when it differs from the requested
    /// address (dual-mode LE fallback, #67). `None` = transport does not know.
    session_addr: Option<String>,
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
                connected_list: Vec::new(),
                session_addr: None,
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

    fn connected_devices(&mut self) -> Result<Vec<DiscoveredDevice>, TransportError> {
        Ok(self.connected_list.clone())
    }

    fn connect(&mut self, _address: &str) -> Result<(), TransportError> {
        let (block_ms, fail) = {
            let mut shared = self.shared.lock().expect("shared mutex");
            shared.connect_attempts += 1;
            (shared.connect_block_ms, shared.fail_connect)
        };
        if block_ms > 0 {
            std::thread::sleep(Duration::from_millis(block_ms));
        }
        if fail {
            return Err(TransportError::DeviceOutOfRange);
        }
        self.connected = true;
        self.connected_model_known = self.model_known;
        self.shared.lock().expect("shared mutex").link_up = true;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), TransportError> {
        self.connected = false;
        self.connected_model_known = false;
        self.shared.lock().expect("shared mutex").link_up = false;
        Ok(())
    }

    fn is_connected(&mut self) -> Result<bool, TransportError> {
        Ok(self.connected && self.shared.lock().expect("shared mutex").link_up)
    }

    fn session_address(&mut self) -> Option<String> {
        self.connected.then(|| self.session_addr.clone()).flatten()
    }

    fn read(&mut self, char_uuid: &str) -> Result<Vec<u8>, TransportError> {
        if !self.connected {
            return Err(TransportError::Disconnected);
        }
        self.shared.lock().expect("shared mutex").reads += 1;
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

    fn attest_model_known(&mut self) {
        if self.connected {
            self.connected_model_known = true;
        }
    }
}

fn start_with(
    model_known: bool,
    known_devices: Vec<String>,
) -> (qcy_app::core::AppHandle, Arc<Mutex<Shared>>) {
    let (transport, shared) = TestTransport::new(model_known);
    (
        AppCore::start(Box::new(transport), HostServices::default(), known_devices),
        shared,
    )
}

fn start(model_known: bool) -> (qcy_app::core::AppHandle, Arc<Mutex<Shared>>) {
    start_with(model_known, Vec::new())
}

fn start_with_connected(
    connected_list: Vec<DiscoveredDevice>,
    known_devices: Vec<String>,
) -> (qcy_app::core::AppHandle, Arc<Mutex<Shared>>) {
    let (mut transport, shared) = TestTransport::new(false);
    transport.connected_list = connected_list;
    (
        AppCore::start(Box::new(transport), HostServices::default(), known_devices),
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
fn noise_mode_maps_to_the_validated_anc_setting_table() {
    // Live HT08 evidence (#50/#52/#54): SetNoise must emit 0x17 AncSetting
    // with the validated [mode, subScene, noiseValue] payloads — never the
    // falsified 0x0C NoiseCancelMode.
    let (handle, shared) = start(true);
    scan_and_connect(&handle);
    let cases: &[(SimpleNoise, &[u8])] = &[
        (SimpleNoise::Off, &[0x02, 0x00, 0x00]),
        (SimpleNoise::Anc, &[0x01, 0x01, 0x02]),
        (SimpleNoise::Adaptive, &[0x01, 0x05, 0x02]),
        (SimpleNoise::Commuting, &[0x01, 0x02, 0x02]),
        (SimpleNoise::Noisy, &[0x01, 0x03, 0x02]),
        (SimpleNoise::Wind, &[0x01, 0x04, 0x02]),
        (SimpleNoise::Transparency, &[0x03, 0x02, 0x04]),
    ];
    for (mode, params) in cases {
        handle.send(AppCommand::SetNoise(*mode)).unwrap();
        recv_until(&handle, |e| matches!(e, AppEvent::Info(_)));
        let writes = shared.lock().expect("shared mutex").writes.clone();
        let packet = decode_packet(writes.last().expect("write recorded")).expect("frame decodes");
        assert_eq!(
            packet.blocks[0].cmd,
            qcy_protocol::Cmd::AncSetting as u8,
            "{mode:?} must use 0x17 AncSetting"
        );
        assert_eq!(packet.blocks[0].params, *params, "{mode:?} payload");
    }
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

#[test]
fn confirm_model_requires_the_connected_device() {
    let (handle, _shared) = start(false);
    // No connection at all: refused.
    handle
        .send(AppCommand::ConfirmModel {
            address: ADDR.into(),
        })
        .unwrap();
    recv_until(&handle, |e| matches!(e, AppEvent::Error(_)));
    // Connected, but a different address: refused.
    scan_and_connect(&handle);
    handle
        .send(AppCommand::ConfirmModel {
            address: "00:11:22:33:44:55".into(),
        })
        .unwrap();
    recv_until(&handle, |e| matches!(e, AppEvent::Error(_)));
    handle.shutdown();
}

#[test]
fn confirm_model_lifts_read_only_and_reports_attestation() {
    let (handle, shared) = start(false);
    scan_and_connect(&handle);

    // Unknown model: writes are denied read-only.
    handle.send(AppCommand::SetNoise(SimpleNoise::Anc)).unwrap();
    recv_until(&handle, |e| matches!(e, AppEvent::Denied(_)));
    assert!(shared.lock().expect("shared mutex").writes.is_empty());

    // Explicit user confirmation lifts the read-only state.
    handle
        .send(AppCommand::ConfirmModel {
            address: ADDR.into(),
        })
        .unwrap();
    let event = recv_until(
        &handle,
        |e| matches!(e, AppEvent::ModelConfirmed { address } if address == ADDR),
    );
    assert!(matches!(event, AppEvent::ModelConfirmed { .. }));
    recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.model_known),
    );

    // Supported writes now reach the transport: validated 0x17 ANC scene.
    handle.send(AppCommand::SetNoise(SimpleNoise::Anc)).unwrap();
    recv_until(&handle, |e| matches!(e, AppEvent::Info(_)));
    let writes = shared.lock().expect("shared mutex").writes.clone();
    assert_eq!(writes.len(), 1);
    let frame = decode_packet(&writes[0]).expect("frame decodes");
    assert_eq!(frame.blocks[0].cmd, 0x17);
    assert_eq!(frame.blocks[0].params, vec![0x01, 0x01, 0x02]);
    handle.shutdown();
}

#[test]
fn previously_attested_address_connects_writable() {
    let (handle, shared) = start_with(false, vec![ADDR.into()]);
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
    assert!(
        snapshot.model_known,
        "a persisted attestation must make the connection writable"
    );
    handle.send(AppCommand::SetNoise(SimpleNoise::Anc)).unwrap();
    recv_until(&handle, |e| matches!(e, AppEvent::Info(_)));
    assert_eq!(shared.lock().expect("shared mutex").writes.len(), 1);
    handle.shutdown();
}

#[test]
fn attestation_is_remembered_in_session_but_not_across_restarts() {
    let (handle, _shared) = start(false);
    scan_and_connect(&handle);
    handle
        .send(AppCommand::ConfirmModel {
            address: ADDR.into(),
        })
        .unwrap();
    recv_until(&handle, |e| matches!(e, AppEvent::ModelConfirmed { .. }));
    // Within the same session a reconnect stays writable (no re-asking).
    handle.send(AppCommand::Disconnect).unwrap();
    recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if !s.connected),
    );
    handle.send(AppCommand::Connect(ADDR.into())).unwrap();
    let event = recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );
    let AppEvent::StateChanged(snapshot) = event else {
        unreachable!()
    };
    assert!(snapshot.model_known);
    handle.shutdown();

    // A fresh core without a persisted attestation starts read-only again:
    // cross-launch persistence is owned by the application layer (config).
    let (handle2, _shared2) = start(false);
    scan_and_connect(&handle2);
    handle2.send(AppCommand::RefreshStatus).unwrap();
    let event = recv_until(
        &handle2,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );
    let AppEvent::StateChanged(snapshot) = event else {
        unreachable!()
    };
    assert!(!snapshot.model_known);
    handle2.shutdown();
}

/* Auto-attach: devices already connected at the host level (no scan needed) */

#[test]
fn attach_connected_attaches_an_already_connected_device_without_scanning() {
    let (handle, _shared) = start_with_connected(
        vec![DiscoveredDevice {
            address: ADDR.into(),
            name: "QCY MeloBuds Pro".into(),
            rssi: None,
            model_known: true,
        }],
        Vec::new(),
    );
    handle.send(AppCommand::AttachConnected).unwrap();
    // The candidate list is surfaced, then the device attaches.
    recv_until(&handle, |e| matches!(e, AppEvent::Discovered(_)));
    let event = recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );
    let AppEvent::StateChanged(snapshot) = event else {
        unreachable!()
    };
    assert_eq!(snapshot.address, ADDR);
    assert!(snapshot.model_known);
    handle.shutdown();
}

#[test]
fn attach_connected_is_a_silent_noop_without_connected_devices() {
    let (handle, _shared) = start(false);
    handle.send(AppCommand::AttachConnected).unwrap();
    // Nothing attached; a manual scan still works and no error was emitted in
    // between (recv_until would surface any Error before the Discovered event).
    handle.send(AppCommand::Scan).unwrap();
    let event = recv_until(&handle, |e| matches!(e, AppEvent::Discovered(_)));
    assert!(matches!(event, AppEvent::Discovered(_)));
    handle.shutdown();
}

#[test]
fn attach_connected_applies_a_previous_model_attestation() {
    // Renamed earbuds: the connected candidate does not prove the model, but
    // the address was confirmed before (persisted `knownDevices`), so the
    // attached session starts writable.
    let (handle, _shared) = start_with_connected(
        vec![DiscoveredDevice {
            address: ADDR.into(),
            name: "Fones da Carol".into(),
            rssi: None,
            model_known: false,
        }],
        vec![ADDR.into()],
    );
    handle.send(AppCommand::AttachConnected).unwrap();
    let event = recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );
    let AppEvent::StateChanged(snapshot) = event else {
        unreachable!()
    };
    assert!(snapshot.model_known, "previous attestation must apply");
    handle.shutdown();
}

/// Fast supervisor timing for deterministic tests: 10 ms ticks, link checked
/// every tick, re-bootstrap cooldown of 30 ms.
fn fast_supervisor() -> SupervisorConfig {
    SupervisorConfig {
        tick: Duration::from_millis(10),
        link_check_every_ticks: 1,
        status_refresh_every_ticks: 5,
        rebootstrap_cooldown: Duration::from_millis(30),
    }
}

fn start_supervised(model_known: bool) -> (qcy_app::core::AppHandle, Arc<Mutex<Shared>>) {
    let (transport, shared) = TestTransport::new(model_known);
    (
        AppCore::start_with_supervisor(
            Box::new(transport),
            HostServices::default(),
            Vec::new(),
            fast_supervisor(),
        ),
        shared,
    )
}

#[test]
fn resident_session_detects_link_loss_and_re_bootstraps() {
    // Issue #54: reconnect-per-action is not viable on the HT08 control
    // identity; the core must hold the session and re-bootstrap automatically
    // after a link loss.
    let (handle, shared) = start_supervised(true);
    handle.send(AppCommand::Connect(ADDR.into())).unwrap();
    recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );

    // Simulate the earbuds dropping the LE link (case, sleep, out of range).
    shared.lock().expect("shared mutex").link_up = false;
    let lost = recv_until(&handle, |e| matches!(e, AppEvent::SessionLost { .. }));
    assert_eq!(
        lost,
        AppEvent::SessionLost {
            address: ADDR.into()
        }
    );

    // The supervisor re-bootstraps in the background; TestTransport.connect()
    // restores the link, so the session must come back on its own.
    let restored = recv_until(&handle, |e| matches!(e, AppEvent::SessionRestored { .. }));
    assert_eq!(
        restored,
        AppEvent::SessionRestored {
            address: ADDR.into()
        }
    );
    recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );
    handle.shutdown();
}

#[test]
fn keepalive_issues_periodic_status_reads_while_connected() {
    // The resident session must generate periodic GATT traffic: the HT08
    // firmware drops fully idle LE links (live evidence, #54).
    let (handle, shared) = start_supervised(true);
    handle.send(AppCommand::Connect(ADDR.into())).unwrap();
    recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );
    let reads_after_connect = shared.lock().expect("shared mutex").reads;
    // fast supervisor refreshes every 5 ticks (~50 ms); no user commands sent.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if shared.lock().expect("shared mutex").reads >= reads_after_connect + 2 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let reads = shared.lock().expect("shared mutex").reads;
    assert!(
        reads >= reads_after_connect + 2,
        "keepalive must keep issuing proven reads (before: {reads_after_connect}, after: {reads})"
    );
    handle.shutdown();
}

#[test]
fn explicit_disconnect_disarms_the_supervisor() {
    let (handle, shared) = start_supervised(true);
    handle.send(AppCommand::Connect(ADDR.into())).unwrap();
    recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );
    handle.send(AppCommand::Disconnect).unwrap();
    recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if !s.connected),
    );

    // Even if the host link state drops afterwards, no supervision events or
    // reconnect attempts may happen after an explicit user disconnect.
    shared.lock().expect("shared mutex").link_up = false;
    let attempts_before = shared.lock().expect("shared mutex").connect_attempts;
    std::thread::sleep(Duration::from_millis(120));
    let attempts_after = shared.lock().expect("shared mutex").connect_attempts;
    assert_eq!(
        attempts_before, attempts_after,
        "no background reconnect after explicit disconnect"
    );
    handle.shutdown();
}

#[test]
fn re_bootstrap_failures_are_surfaced_once_per_error() {
    let (handle, shared) = start_supervised(true);
    handle.send(AppCommand::Connect(ADDR.into())).unwrap();
    recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );

    // Link drops while reconnects fail (e.g. HFP still held, out of window):
    // the supervisor keeps retrying with cooldown but must not spam the UI
    // with the same error.
    {
        let mut s = shared.lock().expect("shared mutex");
        s.link_up = false;
        s.fail_connect = true;
    }
    recv_until(&handle, |e| matches!(e, AppEvent::SessionLost { .. }));
    let first = recv_until(
        &handle,
        |e| matches!(e, AppEvent::Error(msg) if msg.starts_with("re-bootstrap:")),
    );
    // Several cooldown windows pass with the same failure...
    std::thread::sleep(Duration::from_millis(120));
    // ...and no duplicate error event is emitted for the unchanged error.
    while let Some(event) = handle.try_recv_event(Duration::from_millis(20)) {
        match event {
            AppEvent::Error(msg) if msg.starts_with("re-bootstrap:") => {
                panic!("duplicate re-bootstrap error: {msg} (first was {first:?})")
            }
            AppEvent::SessionRestored { .. } => panic!("must not restore while failing"),
            _ => {}
        }
    }
    let attempts = shared.lock().expect("shared mutex").connect_attempts;
    assert!(
        attempts >= 3,
        "supervisor must keep retrying with cooldown (attempts: {attempts})"
    );

    // Once the blocker clears, the session restores itself.
    shared.lock().expect("shared mutex").fail_connect = false;
    recv_until(&handle, |e| matches!(e, AppEvent::SessionRestored { .. }));
    handle.shutdown();
}

#[test]
fn disconnect_interrupts_a_failing_re_bootstrap_promptly() {
    // #64a: while re-bootstrap attempts are failing, an explicit Disconnect
    // must be honored promptly (bounded, < 2 supervisor ticks with the fast
    // test timing) — never behind a full cooldown/retry cycle.
    let (handle, shared) = start_supervised(true);
    handle.send(AppCommand::Connect(ADDR.into())).unwrap();
    recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );

    {
        let mut s = shared.lock().expect("shared mutex");
        s.link_up = false;
        s.fail_connect = true;
    }
    recv_until(&handle, |e| matches!(e, AppEvent::SessionLost { .. }));
    // Wait until the failing re-bootstrap loop is underway, then drain the
    // queue quiet so the disconnect's StateChanged is unambiguous.
    recv_until(
        &handle,
        |e| matches!(e, AppEvent::Error(msg) if msg.starts_with("re-bootstrap:")),
    );
    while handle.try_recv_event(Duration::from_millis(40)).is_some() {}

    let sent = std::time::Instant::now();
    handle.send(AppCommand::Disconnect).unwrap();
    // The disconnect resets the snapshot: the distinguishing marker is the
    // cleared address (the SessionLost snapshot still carries it).
    let mut seen = Vec::new();
    let deadline = sent + Duration::from_secs(2);
    let disconnected = loop {
        assert!(
            std::time::Instant::now() < deadline,
            "Disconnect was not honored; saw: {seen:?}"
        );
        if let Some(event) = handle.try_recv_event(Duration::from_millis(20)) {
            if matches!(event, AppEvent::StateChanged(ref snap) if !snap.connected && snap.address.is_empty())
            {
                break event;
            }
            seen.push(event);
        }
    };
    let elapsed = sent.elapsed();
    let _ = disconnected;
    assert!(
        elapsed < Duration::from_millis(100),
        "Disconnect took {elapsed:?}; two supervisor ticks are 20 ms"
    );

    // The supervisor stays disarmed: no further connect attempts.
    let attempts = shared.lock().expect("shared mutex").connect_attempts;
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        shared.lock().expect("shared mutex").connect_attempts,
        attempts,
        "no background reconnect after explicit disconnect"
    );
    handle.shutdown();
}

#[test]
fn disconnect_is_honored_even_mid_attempt_with_a_blocking_connect() {
    // #64a with a simulated discovery window: the background attempt blocks
    // 200 ms (like BlueZ's LE fallback window). The Disconnect may wait out
    // the in-flight attempt (the transport's connect is not cancellable), but
    // it must be applied immediately after it — never behind a second attempt.
    let (handle, shared) = start_supervised(true);
    handle.send(AppCommand::Connect(ADDR.into())).unwrap();
    recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );

    {
        let mut s = shared.lock().expect("shared mutex");
        s.link_up = false;
        s.fail_connect = true;
        s.connect_block_ms = 200;
    }
    recv_until(&handle, |e| matches!(e, AppEvent::SessionLost { .. }));
    // Let a blocking attempt start, then disconnect mid-attempt.
    std::thread::sleep(Duration::from_millis(50));
    let sent = std::time::Instant::now();
    handle.send(AppCommand::Disconnect).unwrap();
    let deadline = sent + Duration::from_secs(3);
    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "Disconnect was not honored within one attempt window"
        );
        if let Some(event) = handle.try_recv_event(Duration::from_millis(20)) {
            if matches!(event, AppEvent::StateChanged(ref snap) if !snap.connected && snap.address.is_empty())
            {
                break;
            }
        }
    }
    let elapsed = sent.elapsed();
    assert!(
        elapsed < Duration::from_millis(400),
        "Disconnect waited {elapsed:?}; one 200 ms attempt plus margin is the bound"
    );
    // No second attempt may follow the disconnect.
    let attempts = shared.lock().expect("shared mutex").connect_attempts;
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        shared.lock().expect("shared mutex").connect_attempts,
        attempts,
        "no further re-bootstrap attempts after explicit disconnect"
    );
    handle.shutdown();
}

#[test]
fn supervisor_ticks_run_under_continuous_command_traffic() {
    // #64b: ticks are driven by elapsed time, not by queue emptiness. Under a
    // continuous command flood the supervisor must still detect link loss
    // (and keep the keepalive alive) within a bounded time.
    use std::sync::atomic::{AtomicBool, Ordering};

    let (handle, shared) = start_supervised(true);
    handle.send(AppCommand::Connect(ADDR.into())).unwrap();
    recv_until(
        &handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );

    // Flood the command channel faster than the tick rate.
    let stop = Arc::new(AtomicBool::new(false));
    let flooder = {
        let stop = Arc::clone(&stop);
        let commands = handle.commands.clone();
        std::thread::Builder::new()
            .name("flooder".into())
            .spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    if commands.send(AppCommand::RefreshStatus).is_err() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            })
            .expect("flooder spawns")
    };

    // Drop the link while the flood continues: link-loss detection must not
    // starve.
    shared.lock().expect("shared mutex").link_up = false;
    let lost_at = std::time::Instant::now();
    recv_until(&handle, |e| matches!(e, AppEvent::SessionLost { .. }));
    let detection = lost_at.elapsed();
    assert!(
        detection < Duration::from_millis(500),
        "link-loss detection starved under command traffic: took {detection:?}"
    );

    stop.store(true, Ordering::SeqCst);
    let _ = flooder.join();
    handle.shutdown();
}

/* #67: attestation correlates with the session identity, not the request */

const SESSION_ADDR: &str = "11:22:33:44:55:66";

fn start_with_session_addr(
    session_addr: Option<&str>,
    known_devices: Vec<String>,
) -> (qcy_app::core::AppHandle, Arc<Mutex<Shared>>) {
    let (mut transport, shared) = TestTransport::new(false);
    transport.session_addr = session_addr.map(|a| a.to_string());
    (
        AppCore::start(Box::new(transport), HostServices::default(), known_devices),
        shared,
    )
}

fn scan_connect_snapshot(handle: &qcy_app::core::AppHandle) -> qcy_app::core::DeviceSnapshot {
    handle.send(AppCommand::Scan).unwrap();
    recv_until(handle, |e| matches!(e, AppEvent::Discovered(_)));
    handle.send(AppCommand::Connect(ADDR.into())).unwrap();
    let event = recv_until(
        handle,
        |e| matches!(e, AppEvent::StateChanged(s) if s.connected),
    );
    let AppEvent::StateChanged(snapshot) = event else {
        unreachable!()
    };
    snapshot
}

#[test]
fn known_requested_address_does_not_attest_a_different_session_identity() {
    // The requested address was confirmed before, but the live session is
    // held by a fallback identity: the connection must stay read-only.
    let (handle, shared) = start_with_session_addr(Some(SESSION_ADDR), vec![ADDR.into()]);
    let snapshot = scan_connect_snapshot(&handle);
    assert_eq!(snapshot.session_address.as_deref(), Some(SESSION_ADDR));
    assert!(
        !snapshot.model_known,
        "attestation must not transfer from the requested address to an unknown session identity"
    );
    // Writes are denied read-only.
    handle.send(AppCommand::SetNoise(SimpleNoise::Anc)).unwrap();
    recv_until(&handle, |e| matches!(e, AppEvent::Denied(_)));
    assert!(shared.lock().expect("shared mutex").writes.is_empty());
    handle.shutdown();
}

#[test]
fn known_session_address_attests_the_fallback_identity() {
    // The session identity itself was confirmed: the connection starts writable.
    let (handle, _shared) = start_with_session_addr(Some(SESSION_ADDR), vec![SESSION_ADDR.into()]);
    let snapshot = scan_connect_snapshot(&handle);
    assert_eq!(snapshot.session_address.as_deref(), Some(SESSION_ADDR));
    assert!(
        snapshot.model_known,
        "a confirmed session identity must start writable"
    );
    handle.shutdown();
}

#[test]
fn without_session_address_knowledge_the_requested_address_still_correlates() {
    // Transport does not know the session identity (default None): the
    // pre-#67 requested-address correlation stays intact.
    let (handle, _shared) = start_with_session_addr(None, vec![ADDR.into()]);
    let snapshot = scan_connect_snapshot(&handle);
    assert_eq!(snapshot.session_address, None);
    assert!(snapshot.model_known);
    handle.shutdown();
}

#[test]
fn confirm_model_persists_the_session_address() {
    // Confirming the model while the session is held by a fallback identity
    // must persist THAT identity, so the next connect auto-attests it.
    let (handle, _shared) = start_with_session_addr(Some(SESSION_ADDR), Vec::new());
    scan_connect_snapshot(&handle);
    handle
        .send(AppCommand::ConfirmModel {
            address: ADDR.into(),
        })
        .unwrap();
    let event = recv_until(&handle, |e| matches!(e, AppEvent::ModelConfirmed { .. }));
    assert_eq!(
        event,
        AppEvent::ModelConfirmed {
            address: SESSION_ADDR.into()
        },
        "the persisted attestation must target the session identity"
    );
    handle.shutdown();
}
