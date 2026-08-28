//! Typed application core (issue #8).
//!
//! The UI (Slint GUI or CLI) never touches GATT bytes: it sends [`AppCommand`]s and
//! receives [`AppEvent`]s carrying typed snapshots. The core owns a
//! [`qcy_transport::Transport`] on a dedicated worker thread plus the [`qcy_host`]
//! services. Write authorization is NOT reimplemented here — every device write is
//! encoded by [`qcy_protocol`] and then converges on the transport's central
//! [`qcy_transport::policy::WritePolicy`], exactly like every other caller.
//!
//! Safety gates owned by this layer:
//!
//! * Find Earbuds requires an explicit `confirmed_not_worn` preflight flag (issue #9
//!   mirror); without it the chime is refused before any write is attempted.
//! * Destructive opcodes are never constructible from [`AppCommand`] and are rejected
//!   by the policy below anyway (defense in depth).
//! * Unknown/generic devices stay read-only: the core exposes `model_known` and the
//!   policy denies their writes.

use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;

use qcy_host::codec::CodecInfo;
use qcy_host::mpris::MediaStatus;
use qcy_host::system_eq::SystemEqStatus;
use qcy_protocol::packet::encode_command;
use qcy_protocol::{BatteryState, Cmd};
use qcy_transport::policy::CHAR_SETTINGS_NOTIFY;
use qcy_transport::{normalize_address, DiscoveredDevice, Transport, TransportError};

/// Characteristic UUIDs used for proven reads (see docs/PROTOCOL.md).
pub const CHAR_BATTERY: &str = "00000008-0000-1000-8000-00805f9b34fb";
pub const CHAR_VERSION: &str = "00000007-0000-1000-8000-00805f9b34fb";

/// Typed battery snapshot for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BatterySnapshot {
    pub left: u8,
    pub right: u8,
    pub case: u8,
    pub charging_left: bool,
    pub charging_right: bool,
    pub charging_case: bool,
}

/// Typed device state snapshot for the UI. Fields are `None`/empty when unknown —
/// never mocked below this boundary.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DeviceSnapshot {
    pub connected: bool,
    pub name: String,
    pub address: String,
    /// The identity actually holding the live session, when the transport
    /// knows it and it differs from the requested address (dual-mode LE
    /// fallback, #67). Attestations correlate with this identity.
    pub session_address: Option<String>,
    /// False when the model is not proven from advertisement/name evidence; such
    /// devices stay read-only.
    pub model_known: bool,
    pub rssi: Option<i16>,
    pub battery: Option<BatterySnapshot>,
    pub firmware: Option<String>,
    /// Last applied ANC scene. `None` until the first successful write in this
    /// session (the device reports ANC state only through notifications, which
    /// the native transport does not surface yet).
    pub noise: Option<SimpleNoise>,
}

/// Noise-control modes mapped to the hardware-validated `AncSetting` (0x17)
/// scene table (live HT08 evidence, #50/#52/#54).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimpleNoise {
    Off,
    /// ANC indoor — validated payload (1,1,2).
    Anc,
    /// Adaptive ANC — validated payload (1,5,2), ACK (1,5,0).
    Adaptive,
    /// ANC commuting/working — validated payload (1,2,2).
    Commuting,
    /// ANC noisy environment — validated payload (1,3,2).
    Noisy,
    /// ANC wind reduction — validated payload (1,4,2), ACK (1,4,0).
    Wind,
    /// Transparency — validated payload (3,2,4), ACK (3,2,0).
    Transparency,
}

impl SimpleNoise {
    /// Hardware-validated HT08 ANC scene (live evidence, #50/#52/#54):
    /// `[mode, subScene, noiseValue]` for opcode 0x17 AncSetting.
    fn scene(self) -> (u8, u8, u8) {
        match self {
            SimpleNoise::Off => (0x02, 0x00, 0x00),
            SimpleNoise::Anc => (0x01, 0x01, 0x02),
            SimpleNoise::Adaptive => (0x01, 0x05, 0x02),
            SimpleNoise::Commuting => (0x01, 0x02, 0x02),
            SimpleNoise::Noisy => (0x01, 0x03, 0x02),
            SimpleNoise::Wind => (0x01, 0x04, 0x02),
            SimpleNoise::Transparency => (0x03, 0x02, 0x04),
        }
    }
}

/// Raw ANC scene selection mapped to `AncSetting` (0x17) with the
/// hardware-validated `[mode, subScene, noiseValue]` table (see
/// `SimpleNoise::scene`; this struct exposes the raw bytes for advanced use).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AncScene {
    pub mode: u8,
    pub sub_scene: u8,
    pub noise_value: u8,
}

/// Media control actions (host MPRIS, never device writes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCommand {
    Play,
    Pause,
    Next,
    Previous,
}

/// Typed commands from the UI. Deliberately cannot express destructive opcodes,
/// arbitrary opcode writes, or un-preflighted chimes.
#[derive(Debug, Clone, PartialEq)]
pub enum AppCommand {
    Scan,
    Connect(String),
    /// Attach a device that is already connected at the host level (e.g. the
    /// earbuds were connected for audio before the application started).
    /// Lists the already-connected candidates, then attaches the first one —
    /// no redundant link setup, no user-initiated scan needed. No-op when
    /// nothing is connected or the app is already attached.
    AttachConnected,
    /// Explicit user attestation that the currently connected device is a known
    /// model (HT08). Only valid for the connected address; lifts the read-only
    /// state and is reported via [`AppEvent::ModelConfirmed`] so the application
    /// layer can persist it (local-only config field `knownDevices`).
    ConfirmModel {
        address: String,
    },
    Disconnect,
    RefreshStatus,
    SetNoise(SimpleNoise),
    SetAncScene(AncScene),
    SetGameMode(bool),
    SetSleepMode(bool),
    SetInEarDetection(bool),
    /// Find Earbuds. `confirmed_not_worn` must come from the interactive preflight
    /// (issue #9); the core refuses the chime without it.
    FindChime {
        led: bool,
        chime: bool,
        tone_id: u8,
        confirmed_not_worn: bool,
    },
    /// Session-scoped experimental-write opt-in (never persisted).
    SetExperimentalOptIn(bool),
    // Host services (never device writes):
    MediaStatus,
    MediaControl(MediaCommand),
    CodecStatus,
    SystemEqOn(Vec<f64>),
    SystemEqOff,
    SystemEqStatus,
    Shutdown,
}

/// Typed events to the UI.
#[derive(Debug, Clone, PartialEq)]
pub enum AppEvent {
    Discovered(Vec<DiscoveredDevice>),
    StateChanged(DeviceSnapshot),
    HostMedia(MediaStatus),
    HostCodec(CodecInfo),
    HostSystemEq(SystemEqStatus),
    /// An operation failed; the message is user-actionable.
    Error(String),
    /// A write was denied by the central policy (surfaced distinctly for the UI).
    Denied(String),
    /// The user confirmed the connected device's model; the application layer
    /// should persist the address (local-only) so future connections start
    /// writable.
    ModelConfirmed {
        address: String,
    },
    /// Resident-session supervisor: the device link dropped. The core keeps
    /// re-bootstrapping in the background (bounded attempts with cooldown)
    /// until the user disconnects or the session is restored.
    SessionLost {
        address: String,
    },
    /// Resident-session supervisor: the background re-bootstrap succeeded and
    /// the session is live again.
    SessionRestored {
        address: String,
    },
    Info(String),
}

/// Timing for the resident-session supervisor (link-loss detection and
/// background re-bootstrap). Live HT08 evidence (#50/#52): reconnect-per-action
/// is not viable — the control identity only accepts LE connections during its
/// advertisement windows — so a healthy session must be held and re-bootstrapped
/// automatically after link loss. Defaults are conservative for real hardware;
/// tests inject fast values.
#[derive(Debug, Clone, Copy)]
pub struct SupervisorConfig {
    /// Supervisor tick period. Supervision runs from a monotonic deadline
    /// (`Instant`-based), never from queue emptiness (#64b): continuous
    /// command traffic cannot starve link-loss detection or the keepalive.
    pub tick: std::time::Duration,
    /// While connected, check the link every N ticks.
    pub link_check_every_ticks: u32,
    /// While connected, refresh battery/firmware status every N ticks. This is
    /// also the GATT keepalive: periodic proven reads prevent the earbuds from
    /// classifying the LE link as idle and dropping it.
    pub status_refresh_every_ticks: u32,
    /// Cooldown between automatic re-bootstrap attempts after link loss.
    pub rebootstrap_cooldown: std::time::Duration,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            tick: std::time::Duration::from_secs(1),
            link_check_every_ticks: 5,
            status_refresh_every_ticks: 30,
            rebootstrap_cooldown: std::time::Duration::from_secs(15),
        }
    }
}

/// Optional tracing for the resident-session supervisor and connection
/// lifecycle, enabled with `QCY_CORE_TRACE=1` (also accepts `true`/`yes`,
/// case-insensitive). Used to diagnose live-hardware session behavior without
/// changing shipped behavior.
fn trace(message: &str) {
    if trace_enabled() {
        eprintln!(
            "521c-core: {message} (t={:.1}s)",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0)
        );
    }
}

fn trace_enabled() -> bool {
    std::env::var("QCY_CORE_TRACE").is_ok_and(|v| trace_value_is_truthy(&v))
}

/// Truthy parse for `QCY_CORE_TRACE` (#71): tracing is enabled only for
/// `1` / `true` / `yes` (case-insensitive, trimmed). Mere presence of the
/// variable — including `QCY_CORE_TRACE=0` — does NOT enable it; the
/// documented contract is an explicit truthy value.
fn trace_value_is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes"
    )
}

/// Quiet status refresh used as the resident-session keepalive: updates the
/// snapshot in place and never emits error events (a transient read failure
/// must not spam the UI; link loss is detected via `is_connected`).
fn keepalive_refresh(transport: &mut Box<dyn Transport + Send>, snapshot: &mut DeviceSnapshot) {
    if let Ok(bytes) = transport.read(CHAR_BATTERY) {
        if let Some(state) = BatteryState::decode(&bytes) {
            snapshot.battery = Some(BatterySnapshot {
                left: state.left.level,
                right: state.right.level,
                case: state.case.level,
                charging_left: state.left.charging,
                charging_right: state.right.charging,
                charging_case: state.case.charging,
            });
        }
    }
    if let Ok(bytes) = transport.read(CHAR_VERSION) {
        if bytes.len() >= 3 {
            snapshot.firmware = Some(format!("{}.{}.{}", bytes[0], bytes[1], bytes[2]));
        }
    }
}

fn enable_byte(on: bool) -> u8 {
    if on {
        0x01
    } else {
        0x02
    }
}

/// MPRIS requests: status queries and control actions are distinct operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaRequest {
    Status,
    Control(MediaCommand),
}

/// Host-service callback shapes (aliases keep the struct readable).
pub type MprisFn = Box<dyn FnMut(MediaRequest) -> Result<MediaStatus, String> + Send>;
pub type CodecFn = Box<dyn FnMut() -> CodecInfo + Send>;
pub type SystemEqFn = Box<dyn FnMut(SystemEqCommand) -> Result<SystemEqStatus, String> + Send>;

/// Host-service dependencies injected into the core. Each is optional so builds/tests
/// without a live bus keep working; `None` degrades gracefully (issue #13 contract).
#[derive(Default)]
pub struct HostServices {
    pub mpris: Option<MprisFn>,
    pub codec: Option<CodecFn>,
    pub system_eq: Option<SystemEqFn>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SystemEqCommand {
    On(Vec<f64>),
    Off,
    Status,
}

/// Handle returned by [`AppCore::start`]: send commands, receive events, join on drop.
pub struct AppHandle {
    pub commands: Sender<AppCommand>,
    events: Option<Receiver<AppEvent>>,
    worker: Option<JoinHandle<()>>,
}

impl AppHandle {
    pub fn send(&self, cmd: AppCommand) -> Result<(), String> {
        self.commands
            .send(cmd)
            .map_err(|_| "application core stopped".to_string())
    }
    /// Take ownership of the event receiver (e.g. to move it into a UI pump
    /// thread). `Receiver` is not `Clone`, so this can only be done once.
    pub fn take_events(&mut self) -> Option<Receiver<AppEvent>> {
        self.events.take()
    }

    /// Wait for the next event with a timeout (when the receiver was not taken).
    pub fn try_recv_event(&self, timeout: std::time::Duration) -> Option<AppEvent> {
        let Some(events) = &self.events else {
            return None;
        };
        match events.recv_timeout(timeout) {
            Ok(event) => Some(event),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => None,
        }
    }
    /// Stop the worker and join it.
    pub fn shutdown(mut self) {
        let _ = self.commands.send(AppCommand::Shutdown);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for AppHandle {
    fn drop(&mut self) {
        let _ = self.commands.send(AppCommand::Shutdown);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

/// The application core. Spawns a worker thread owning the transport; the UI thread
/// communicates exclusively through channels.
pub struct AppCore;

impl AppCore {
    /// Start the core. `known_devices` is the list of addresses whose model the
    /// user already confirmed (local-only config field `knownDevices`); matching
    /// connections start writable without re-asking.
    pub fn start(
        transport: Box<dyn Transport + Send>,
        host: HostServices,
        known_devices: Vec<String>,
    ) -> AppHandle {
        Self::start_with_supervisor(transport, host, known_devices, SupervisorConfig::default())
    }

    /// Like [`start`](Self::start) with explicit resident-session supervisor
    /// timing (issue #54).
    pub fn start_with_supervisor(
        transport: Box<dyn Transport + Send>,
        host: HostServices,
        known_devices: Vec<String>,
        supervisor: SupervisorConfig,
    ) -> AppHandle {
        let (cmd_tx, cmd_rx) = channel::<AppCommand>();
        let (event_tx, event_rx) = channel::<AppEvent>();

        let worker = std::thread::Builder::new()
            .name("521c-app-core".into())
            .spawn(move || {
                /// Mutable worker state threaded through the command handler
                /// and the supervisor tick.
                struct WorkerState {
                    transport: Box<dyn Transport + Send>,
                    snapshot: DeviceSnapshot,
                    last_discovered: Vec<DiscoveredDevice>,
                    known_devices: Vec<String>,
                    experimental_opt_in: bool,
                    /// Resident-session supervision (#54): armed on a successful
                    /// connect, disarmed on an explicit user disconnect.
                    supervising: bool,
                    host: HostServices,
                }

                let mut state = WorkerState {
                    transport,
                    snapshot: DeviceSnapshot::default(),
                    last_discovered: Vec::new(),
                    known_devices: known_devices.iter().map(|a| normalize_address(a)).collect(),
                    experimental_opt_in: false,
                    supervising: false,
                    host,
                };
                let mut tick_count: u32 = 0;
                let mut last_rebootstrap: Option<std::time::Instant> = None;
                let mut last_rebootstrap_error: Option<String> = None;

                let emit = |event: AppEvent| {
                    let _ = event_tx.send(event);
                };

                let refresh_status = |transport: &mut Box<dyn Transport + Send>,
                                      snapshot: &mut DeviceSnapshot| {
                    if !snapshot.connected {
                        return;
                    }
                    match transport.read(CHAR_BATTERY) {
                        Ok(bytes) => {
                            if let Some(state) = BatteryState::decode(&bytes) {
                                snapshot.battery = Some(BatterySnapshot {
                                    left: state.left.level,
                                    right: state.right.level,
                                    case: state.case.level,
                                    charging_left: state.left.charging,
                                    charging_right: state.right.charging,
                                    charging_case: state.case.charging,
                                });
                            }
                        }
                        Err(e) => emit(AppEvent::Error(format!("battery read failed: {e}"))),
                    }
                    match transport.read(CHAR_VERSION) {
                        Ok(bytes) => {
                            if bytes.len() >= 3 {
                                snapshot.firmware =
                                    Some(format!("{}.{}.{}", bytes[0], bytes[1], bytes[2]));
                            }
                        }
                        Err(e) => emit(AppEvent::Error(format!("firmware read failed: {e}"))),
                    }
                };

                let write_frame = |transport: &mut Box<dyn Transport + Send>,
                                   frame: Vec<u8>|
                 -> Result<(), AppEvent> {
                    transport.write(&frame).map_err(|e| match e {
                        TransportError::Denied(d) => AppEvent::Denied(format!("{d}")),
                        other => AppEvent::Error(format!("{other}")),
                    })
                };

                // Shared connect path for `Connect` (user-initiated) and
                // `AttachConnected` (already-connected at host level).
                let do_connect = |transport: &mut Box<dyn Transport + Send>,
                                  snapshot: &mut DeviceSnapshot,
                                  last_discovered: &[DiscoveredDevice],
                                  known_devices: &[String],
                                  address: String|
                 -> Result<(), String> {
                    transport
                        .connect(&address)
                        .map_err(|e| format!("connect failed: {e}"))?;
                    // Resident-session keepalive (#54): the HT08 firmware drops
                    // fully idle LE links (live evidence: a notify-subscribed
                    // session survived >1h while an idle app session dropped in
                    // under a minute and looped re-bootstrap). Subscribing the
                    // settings notify keeps the GATT session established and the
                    // device's spontaneous notifications acknowledged.
                    let _ = transport.subscribe(CHAR_SETTINGS_NOTIFY);
                    snapshot.connected = true;
                    snapshot.address = address.clone();
                    if let Some(dev) = last_discovered.iter().find(|d| d.address == address) {
                        snapshot.name = dev.name.clone();
                        snapshot.rssi = dev.rssi;
                        snapshot.model_known = dev.model_known;
                    }
                    // The identity holding the session may differ from the
                    // requested address after a dual-mode LE fallback (#67).
                    // Attestation must correlate with the SESSION identity when
                    // the transport knows it, never with the requested address
                    // alone.
                    let session_address =
                        transport.session_address().map(|a| normalize_address(&a));
                    let attest_address = session_address
                        .clone()
                        .unwrap_or_else(|| normalize_address(&address));
                    snapshot.session_address = session_address;
                    // A previously confirmed model starts writable.
                    if !snapshot.model_known && known_devices.contains(&attest_address) {
                        transport.attest_model_known();
                        snapshot.model_known = true;
                    }
                    refresh_status(transport, snapshot);
                    Ok(())
                };

                // Tag a failed toggle write with the feature name so the UI can
                // roll back exactly the optimistic toggle that caused it (#71).
                // Success `Info` events already carry the same prefix.
                let tag_failure = |event: AppEvent, feature: &'static str| match event {
                    AppEvent::Error(message) => AppEvent::Error(format!("{feature}: {message}")),
                    AppEvent::Denied(message) => AppEvent::Denied(format!("{feature}: {message}")),
                    other => other,
                };

                // Handle one typed command; returns false when the worker must
                // stop (`Shutdown`).
                let handle_command = |cmd: AppCommand, state: &mut WorkerState| -> bool {
                    match cmd {
                        AppCommand::Shutdown => return false,
                        AppCommand::Scan => match state.transport.scan() {
                            Ok(list) => {
                                state.last_discovered = list.clone();
                                emit(AppEvent::Discovered(list));
                            }
                            Err(e) => emit(AppEvent::Error(format!("scan failed: {e}"))),
                        },
                        AppCommand::Connect(address) => {
                            match do_connect(
                                &mut state.transport,
                                &mut state.snapshot,
                                &state.last_discovered,
                                &state.known_devices,
                                address,
                            ) {
                                Ok(()) => {
                                    // A user-established session becomes the
                                    // resident session (#54): the supervisor
                                    // holds it and re-bootstraps on link loss.
                                    trace("user connect succeeded; supervisor armed");
                                    state.supervising = true;
                                    emit(AppEvent::StateChanged(state.snapshot.clone()));
                                }
                                Err(msg) => emit(AppEvent::Error(msg)),
                            }
                        }
                        AppCommand::AttachConnected => {
                            if state.snapshot.connected {
                                return true;
                            }
                            match state.transport.connected_devices() {
                                Ok(list) if list.is_empty() => {
                                    // Nothing connected at host level; the user
                                    // can still scan manually.
                                }
                                Ok(list) => {
                                    // Deterministic choice: the transport sorts
                                    // candidates by address.
                                    let address = list[0].address.clone();
                                    state.last_discovered = list.clone();
                                    emit(AppEvent::Discovered(list));
                                    match do_connect(
                                        &mut state.transport,
                                        &mut state.snapshot,
                                        &state.last_discovered,
                                        &state.known_devices,
                                        address,
                                    ) {
                                        Ok(()) => {
                                            state.supervising = true;
                                            emit(AppEvent::StateChanged(state.snapshot.clone()))
                                        }
                                        Err(msg) => emit(AppEvent::Error(msg)),
                                    }
                                }
                                Err(e) => {
                                    emit(AppEvent::Error(format!("attach failed: {e}")))
                                }
                            }
                        }
                        AppCommand::ConfirmModel { address } => {
                            let normalized = normalize_address(&address);
                            if !state.snapshot.connected
                                || normalize_address(&state.snapshot.address) != normalized
                            {
                                emit(AppEvent::Error(
                                    "model confirmation requires the device to be connected"
                                        .to_string(),
                                ));
                                return true;
                            }
                            state.transport.attest_model_known();
                            state.snapshot.model_known = true;
                            // Persist the identity that actually holds the
                            // session (#67): the next connect must auto-attest
                            // the session identity, not the requested address
                            // that may have fallen back.
                            let persist_address = state
                                .snapshot
                                .session_address
                                .clone()
                                .unwrap_or(normalized);
                            if !state.known_devices.contains(&persist_address) {
                                state.known_devices.push(persist_address.clone());
                            }
                            emit(AppEvent::ModelConfirmed {
                                address: persist_address,
                            });
                            emit(AppEvent::StateChanged(state.snapshot.clone()));
                        }
                        AppCommand::Disconnect => {
                            // Explicit user disconnect disarms the resident
                            // session; no background re-bootstrap after it.
                            trace("user disconnect; supervisor disarmed");
                            state.supervising = false;
                            let _ = state.transport.disconnect();
                            state.snapshot = DeviceSnapshot::default();
                            emit(AppEvent::StateChanged(state.snapshot.clone()));
                        }
                        AppCommand::RefreshStatus => {
                            refresh_status(&mut state.transport, &mut state.snapshot);
                            emit(AppEvent::StateChanged(state.snapshot.clone()));
                        }
                        AppCommand::SetNoise(mode) => {
                            // Live HT08 evidence (#50/#52): 0x0C NoiseCancelMode
                            // is ignored by the device; ANC state is set through
                            // 0x17 AncSetting with the validated scene table.
                            let (m, sub, noise) = mode.scene();
                            match encode_command(Cmd::AncSetting as u8, &[m, sub, noise]) {
                                Ok(frame) => match write_frame(&mut state.transport, frame) {
                                    Ok(()) => {
                                        // Record the applied scene so the UI can
                                        // reflect it. The device confirms writes
                                        // through notify ACKs (same frame), which
                                        // the native transport does not surface
                                        // yet; a successful GATT write is the
                                        // best available signal.
                                        state.snapshot.noise = Some(mode);
                                        emit(AppEvent::Info(format!(
                                            "noise mode set to {mode:?}"
                                        )));
                                        emit(AppEvent::StateChanged(state.snapshot.clone()));
                                    }
                                    Err(event) => emit(event),
                                },
                                Err(e) => emit(AppEvent::Error(format!("encode failed: {e:?}"))),
                            }
                        }
                        AppCommand::SetAncScene(scene) => {
                            match encode_command(
                                Cmd::AncSetting as u8,
                                &[scene.mode, scene.sub_scene, scene.noise_value],
                            ) {
                                Ok(frame) => match write_frame(&mut state.transport, frame) {
                                    Ok(()) => emit(AppEvent::Info("ANC scene applied".into())),
                                    Err(event) => emit(event),
                                },
                                Err(e) => emit(AppEvent::Error(format!("encode failed: {e:?}"))),
                            }
                        }
                        AppCommand::SetGameMode(on) => {
                            match encode_command(Cmd::LowLatency as u8, &[enable_byte(on)]) {
                                Ok(frame) => match write_frame(&mut state.transport, frame) {
                                    Ok(()) => emit(AppEvent::Info(format!(
                                        "game mode {}",
                                        if on { "on" } else { "off" }
                                    ))),
                                    Err(event) => emit(tag_failure(event, "game mode")),
                                },
                                Err(e) => emit(AppEvent::Error(format!("encode failed: {e:?}"))),
                            }
                        }
                        AppCommand::SetSleepMode(on) => {
                            match encode_command(Cmd::SleepMode as u8, &[enable_byte(on)]) {
                                Ok(frame) => match write_frame(&mut state.transport, frame) {
                                    Ok(()) => emit(AppEvent::Info(format!(
                                        "sleep mode {}",
                                        if on { "on" } else { "off" }
                                    ))),
                                    Err(event) => emit(tag_failure(event, "sleep mode")),
                                },
                                Err(e) => emit(AppEvent::Error(format!("encode failed: {e:?}"))),
                            }
                        }
                        AppCommand::SetInEarDetection(on) => {
                            match encode_command(Cmd::InEarDetection as u8, &[enable_byte(on)]) {
                                Ok(frame) => match write_frame(&mut state.transport, frame) {
                                    Ok(()) => emit(AppEvent::Info(format!(
                                        "in-ear detection {}",
                                        if on { "on" } else { "off" }
                                    ))),
                                    Err(event) => emit(tag_failure(event, "in-ear detection")),
                                },
                                Err(e) => emit(AppEvent::Error(format!("encode failed: {e:?}"))),
                            }
                        }
                        AppCommand::FindChime {
                            led,
                            chime,
                            tone_id,
                            confirmed_not_worn,
                        } => {
                            // Interactive preflight gate (issue #9 mirror): the chime is
                            // refused unless a human confirmed the earbuds are not worn.
                            if !confirmed_not_worn {
                                emit(AppEvent::Denied(
                                    "Find Earbuds requires the interactive preflight: confirm the earbuds are not being worn before sounding the chime.".into(),
                                ));
                                return true;
                            }
                            if led {
                                if let Ok(frame) =
                                    encode_command(Cmd::LightFlash as u8, &[0x01])
                                {
                                    if let Err(event) = write_frame(&mut state.transport, frame) {
                                        emit(event);
                                        return true;
                                    }
                                }
                            }
                            if chime {
                                match encode_command(Cmd::TonePlay as u8, &[tone_id]) {
                                    Ok(frame) => match write_frame(&mut state.transport, frame) {
                                        Ok(()) => emit(AppEvent::Info("chime sent".into())),
                                        Err(event) => emit(event),
                                    },
                                    Err(e) => emit(AppEvent::Error(format!("encode failed: {e:?}"))),
                                }
                            }
                        }
                        AppCommand::SetExperimentalOptIn(on) => {
                            state.experimental_opt_in = on;
                            state.transport.set_experimental_opt_in(on);
                            emit(AppEvent::Info(format!(
                                "experimental writes {}",
                                if on { "enabled for this session" } else { "disabled" }
                            )));
                        }
                        AppCommand::MediaStatus => match state.host.mpris.as_mut() {
                            Some(f) => match f(MediaRequest::Status) {
                                Ok(status) => emit(AppEvent::HostMedia(status)),
                                Err(e) => emit(AppEvent::Error(e)),
                            },
                            None => emit(AppEvent::Error(
                                "MPRIS is not available in this build/session".into(),
                            )),
                        },
                        AppCommand::MediaControl(action) => match state.host.mpris.as_mut() {
                            Some(f) => match f(MediaRequest::Control(action)) {
                                Ok(status) => emit(AppEvent::HostMedia(status)),
                                Err(e) => emit(AppEvent::Error(e)),
                            },
                            None => emit(AppEvent::Error(
                                "MPRIS is not available in this build/session".into(),
                            )),
                        },
                        AppCommand::CodecStatus => match state.host.codec.as_mut() {
                            Some(f) => emit(AppEvent::HostCodec(f())),
                            None => emit(AppEvent::HostCodec(CodecInfo::unknown())),
                        },
                        AppCommand::SystemEqOn(gains) => match state.host.system_eq.as_mut() {
                            Some(f) => match f(SystemEqCommand::On(gains)) {
                                Ok(status) => emit(AppEvent::HostSystemEq(status)),
                                Err(e) => emit(AppEvent::Error(e)),
                            },
                            None => emit(AppEvent::Error(
                                "System EQ is not available in this build".into(),
                            )),
                        },
                        AppCommand::SystemEqOff => match state.host.system_eq.as_mut() {
                            Some(f) => match f(SystemEqCommand::Off) {
                                Ok(status) => emit(AppEvent::HostSystemEq(status)),
                                Err(e) => emit(AppEvent::Error(e)),
                            },
                            None => emit(AppEvent::Error(
                                "System EQ is not available in this build".into(),
                            )),
                        },
                        AppCommand::SystemEqStatus => match state.host.system_eq.as_mut() {
                            Some(f) => match f(SystemEqCommand::Status) {
                                Ok(status) => emit(AppEvent::HostSystemEq(status)),
                                Err(e) => emit(AppEvent::Error(e)),
                            },
                            None => emit(AppEvent::HostSystemEq(SystemEqStatus::default())),
                        },
                    }
                    true
                };

                // Supervision is time-driven (#64b): the tick runs whenever its
                // monotonic deadline has elapsed, regardless of command traffic.
                // The previous design only ticked on an empty command queue, so
                // sustained traffic starved link-loss detection and the GATT
                // keepalive entirely.
                let tick_period = supervisor.tick.max(std::time::Duration::from_millis(1));
                let mut next_tick = std::time::Instant::now() + tick_period;

                loop {
                    let now = std::time::Instant::now();
                    if now >= next_tick {
                        // Advance the deadline in whole periods: missed slots are
                        // skipped instead of bursting catch-up ticks after a long
                        // block.
                        while next_tick <= now {
                            next_tick += tick_period;
                        }
                        tick_count = tick_count.wrapping_add(1);
                        if state.supervising && state.snapshot.connected {
                            if tick_count.is_multiple_of(supervisor.link_check_every_ticks.max(1))
                            {
                                match state.transport.is_connected() {
                                    Ok(true) => {}
                                    Ok(false) => {
                                        trace("link loss detected by supervisor");
                                        state.snapshot.connected = false;
                                        let addr = state.snapshot.address.clone();
                                        emit(AppEvent::SessionLost { address: addr });
                                        emit(AppEvent::StateChanged(state.snapshot.clone()));
                                        last_rebootstrap = None;
                                        last_rebootstrap_error = None;
                                    }
                                    Err(e) => emit(AppEvent::Error(format!(
                                        "link state check failed: {e}"
                                    ))),
                                }
                            }
                            // Keepalive + fresh UI data: periodic proven
                            // reads keep the LE link busy enough that the
                            // earbuds do not drop it as idle.
                            if tick_count
                                .is_multiple_of(supervisor.status_refresh_every_ticks.max(1))
                            {
                                keepalive_refresh(&mut state.transport, &mut state.snapshot);
                                emit(AppEvent::StateChanged(state.snapshot.clone()));
                            }
                        } else if state.supervising && !state.snapshot.connected {
                            // Re-bootstrap mode: retry the connection with a
                            // cooldown until the user disconnects. Errors are
                            // only surfaced when they change, so a long
                            // out-of-window stretch does not spam the UI.
                            let due = last_rebootstrap
                                .is_none_or(|t| t.elapsed() >= supervisor.rebootstrap_cooldown);
                            if due {
                                last_rebootstrap = Some(std::time::Instant::now());
                                let address = state.snapshot.address.clone();
                                trace(&format!("re-bootstrap attempt for {address}"));
                                // Run the blocking connect attempt on a scoped
                                // helper thread and keep draining the command
                                // channel while it runs (#64a): an explicit
                                // Disconnect must not wait a full discovery
                                // window behind a background re-bootstrap.
                                let mut queued: Vec<AppCommand> = Vec::new();
                                let mut user_disconnected = false;
                                let mut shutdown_requested = false;
                                let result = std::thread::scope(|s| {
                                    let attempt = s.spawn(|| {
                                        do_connect(
                                            &mut state.transport,
                                            &mut state.snapshot,
                                            &state.last_discovered,
                                            &state.known_devices,
                                            address.clone(),
                                        )
                                    });
                                    while !attempt.is_finished() {
                                        match cmd_rx
                                            .recv_timeout(std::time::Duration::from_millis(20))
                                        {
                                            Ok(AppCommand::Disconnect) => user_disconnected = true,
                                            Ok(AppCommand::Shutdown) => shutdown_requested = true,
                                            Ok(cmd) => queued.push(cmd),
                                            Err(RecvTimeoutError::Timeout) => {}
                                            Err(RecvTimeoutError::Disconnected) => {
                                                shutdown_requested = true;
                                                break;
                                            }
                                        }
                                    }
                                    attempt.join().expect("re-bootstrap attempt joins")
                                });
                                if user_disconnected {
                                    trace("user disconnect during re-bootstrap; supervisor disarmed");
                                    state.supervising = false;
                                    let _ = state.transport.disconnect();
                                    state.snapshot = DeviceSnapshot::default();
                                    emit(AppEvent::StateChanged(state.snapshot.clone()));
                                    if shutdown_requested {
                                        break;
                                    }
                                    // Queued commands targeted the session the
                                    // user just ended; drop them rather than
                                    // replaying into a reset snapshot.
                                    continue;
                                }
                                if shutdown_requested {
                                    break;
                                }
                                match result {
                                    Ok(()) => {
                                        trace("re-bootstrap succeeded");
                                        last_rebootstrap_error = None;
                                        emit(AppEvent::SessionRestored { address });
                                        emit(AppEvent::StateChanged(state.snapshot.clone()));
                                    }
                                    Err(msg) => {
                                        if last_rebootstrap_error.as_deref() != Some(&msg) {
                                            last_rebootstrap_error = Some(msg.clone());
                                            emit(AppEvent::Error(format!(
                                                "re-bootstrap: {msg}"
                                            )));
                                        }
                                    }
                                }
                                // Replay commands that arrived during the
                                // attempt, in order.
                                for queued_cmd in queued {
                                    trace(&format!("replaying queued command: {queued_cmd:?}"));
                                    if !handle_command(queued_cmd, &mut state) {
                                        return;
                                    }
                                }
                            }
                        }
                    }

                    // Wait for the next command, but never past the next tick
                    // deadline (#64b).
                    let wait = next_tick.saturating_duration_since(std::time::Instant::now());
                    let cmd = match cmd_rx.recv_timeout(wait) {
                        Ok(cmd) => {
                            trace(&format!("command received: {cmd:?}"));
                            Some(cmd)
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                        Err(RecvTimeoutError::Timeout) => None,
                    };
                    if let Some(cmd) = cmd {
                        if !handle_command(cmd, &mut state) {
                            break;
                        }
                    }
                }
            })
            .expect("app core thread spawns");

        AppHandle {
            commands: cmd_tx,
            events: Some(event_rx),
            worker: Some(worker),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::trace_value_is_truthy;

    #[test]
    fn qcy_core_trace_is_truthy_parsed_not_mere_presence() {
        // #71: `QCY_CORE_TRACE=0` (or any non-truthy value) must stay silent.
        assert!(trace_value_is_truthy("1"));
        assert!(trace_value_is_truthy("true"));
        assert!(trace_value_is_truthy("TRUE"));
        assert!(trace_value_is_truthy("Yes"));
        assert!(trace_value_is_truthy(" yes "));
        assert!(!trace_value_is_truthy("0"));
        assert!(!trace_value_is_truthy("false"));
        assert!(!trace_value_is_truthy("no"));
        assert!(!trace_value_is_truthy(""));
        assert!(!trace_value_is_truthy("  "));
    }
}
