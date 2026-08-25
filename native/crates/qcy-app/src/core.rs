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
use qcy_transport::{DiscoveredDevice, Transport, TransportError};

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
    /// False when the model is not proven from advertisement/name evidence; such
    /// devices stay read-only.
    pub model_known: bool,
    pub rssi: Option<i16>,
    pub battery: Option<BatterySnapshot>,
    pub firmware: Option<String>,
}

/// Simple noise-cancel modes mapped to `NoiseCancelMode` (0x0C).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimpleNoise {
    Off,
    Anc,
    Outdoor,
    Transparency,
}

impl SimpleNoise {
    fn byte(self) -> u8 {
        match self {
            SimpleNoise::Off => 0x00,
            SimpleNoise::Anc => 0x01,
            SimpleNoise::Outdoor => 0x02,
            SimpleNoise::Transparency => 0x03,
        }
    }
}

/// ANC scene selection mapped to `AncSetting` (0x17): mode 0x02/0x03/0x04 with a
/// 1-3 sub-scene, or transparency 0x0A with a 1-7 level.
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
    Info(String),
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
    pub events: Receiver<AppEvent>,
    worker: Option<JoinHandle<()>>,
}

impl AppHandle {
    pub fn send(&self, cmd: AppCommand) -> Result<(), String> {
        self.commands
            .send(cmd)
            .map_err(|_| "application core stopped".to_string())
    }
    /// Wait for the next event with a timeout (UI pumps this from its event loop).
    pub fn try_recv_event(&self, timeout: std::time::Duration) -> Option<AppEvent> {
        match self.events.recv_timeout(timeout) {
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
    pub fn start(transport: Box<dyn Transport + Send>, mut host: HostServices) -> AppHandle {
        let (cmd_tx, cmd_rx) = channel::<AppCommand>();
        let (event_tx, event_rx) = channel::<AppEvent>();

        let worker = std::thread::Builder::new()
            .name("521c-app-core".into())
            .spawn(move || {
                let mut transport = transport;
                let mut snapshot = DeviceSnapshot::default();
                let mut experimental_opt_in = false;

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

                while let Ok(cmd) = cmd_rx.recv() {
                    match cmd {
                        AppCommand::Shutdown => break,
                        AppCommand::Scan => match transport.scan() {
                            Ok(list) => emit(AppEvent::Discovered(list)),
                            Err(e) => emit(AppEvent::Error(format!("scan failed: {e}"))),
                        },
                        AppCommand::Connect(address) => {
                            match transport.connect(&address) {
                                Ok(()) => {
                                    snapshot.connected = true;
                                    snapshot.address = address;
                                    refresh_status(&mut transport, &mut snapshot);
                                    emit(AppEvent::StateChanged(snapshot.clone()));
                                }
                                Err(e) => emit(AppEvent::Error(format!("connect failed: {e}"))),
                            }
                        }
                        AppCommand::Disconnect => {
                            let _ = transport.disconnect();
                            snapshot = DeviceSnapshot::default();
                            emit(AppEvent::StateChanged(snapshot.clone()));
                        }
                        AppCommand::RefreshStatus => {
                            refresh_status(&mut transport, &mut snapshot);
                            emit(AppEvent::StateChanged(snapshot.clone()));
                        }
                        AppCommand::SetNoise(mode) => {
                            match encode_command(Cmd::NoiseCancelMode as u8, &[mode.byte()]) {
                                Ok(frame) => match write_frame(&mut transport, frame) {
                                    Ok(()) => emit(AppEvent::Info(format!(
                                        "noise mode set to {mode:?}"
                                    ))),
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
                                Ok(frame) => match write_frame(&mut transport, frame) {
                                    Ok(()) => emit(AppEvent::Info("ANC scene applied".into())),
                                    Err(event) => emit(event),
                                },
                                Err(e) => emit(AppEvent::Error(format!("encode failed: {e:?}"))),
                            }
                        }
                        AppCommand::SetGameMode(on) => {
                            match encode_command(Cmd::LowLatency as u8, &[enable_byte(on)]) {
                                Ok(frame) => match write_frame(&mut transport, frame) {
                                    Ok(()) => emit(AppEvent::Info(format!(
                                        "game mode {}",
                                        if on { "on" } else { "off" }
                                    ))),
                                    Err(event) => emit(event),
                                },
                                Err(e) => emit(AppEvent::Error(format!("encode failed: {e:?}"))),
                            }
                        }
                        AppCommand::SetSleepMode(on) => {
                            match encode_command(Cmd::SleepMode as u8, &[enable_byte(on)]) {
                                Ok(frame) => match write_frame(&mut transport, frame) {
                                    Ok(()) => emit(AppEvent::Info(format!(
                                        "sleep mode {}",
                                        if on { "on" } else { "off" }
                                    ))),
                                    Err(event) => emit(event),
                                },
                                Err(e) => emit(AppEvent::Error(format!("encode failed: {e:?}"))),
                            }
                        }
                        AppCommand::SetInEarDetection(on) => {
                            match encode_command(Cmd::InEarDetection as u8, &[enable_byte(on)]) {
                                Ok(frame) => match write_frame(&mut transport, frame) {
                                    Ok(()) => emit(AppEvent::Info(format!(
                                        "in-ear detection {}",
                                        if on { "on" } else { "off" }
                                    ))),
                                    Err(event) => emit(event),
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
                                continue;
                            }
                            if led {
                                if let Ok(frame) =
                                    encode_command(Cmd::LightFlash as u8, &[0x01])
                                {
                                    if let Err(event) = write_frame(&mut transport, frame) {
                                        emit(event);
                                        continue;
                                    }
                                }
                            }
                            if chime {
                                match encode_command(Cmd::TonePlay as u8, &[tone_id]) {
                                    Ok(frame) => match write_frame(&mut transport, frame) {
                                        Ok(()) => emit(AppEvent::Info("chime sent".into())),
                                        Err(event) => emit(event),
                                    },
                                    Err(e) => emit(AppEvent::Error(format!("encode failed: {e:?}"))),
                                }
                            }
                        }
                        AppCommand::SetExperimentalOptIn(on) => {
                            experimental_opt_in = on;
                            transport.set_experimental_opt_in(on);
                            emit(AppEvent::Info(format!(
                                "experimental writes {}",
                                if on { "enabled for this session" } else { "disabled" }
                            )));
                        }
                        AppCommand::MediaStatus => match host.mpris.as_mut() {
                            Some(f) => match f(MediaRequest::Status) {
                                Ok(status) => emit(AppEvent::HostMedia(status)),
                                Err(e) => emit(AppEvent::Error(e)),
                            },
                            None => emit(AppEvent::Error(
                                "MPRIS is not available in this build/session".into(),
                            )),
                        },
                        AppCommand::MediaControl(action) => match host.mpris.as_mut() {
                            Some(f) => match f(MediaRequest::Control(action)) {
                                Ok(status) => emit(AppEvent::HostMedia(status)),
                                Err(e) => emit(AppEvent::Error(e)),
                            },
                            None => emit(AppEvent::Error(
                                "MPRIS is not available in this build/session".into(),
                            )),
                        },
                        AppCommand::CodecStatus => match host.codec.as_mut() {
                            Some(f) => emit(AppEvent::HostCodec(f())),
                            None => emit(AppEvent::HostCodec(CodecInfo::unknown())),
                        },
                        AppCommand::SystemEqOn(gains) => match host.system_eq.as_mut() {
                            Some(f) => match f(SystemEqCommand::On(gains)) {
                                Ok(status) => emit(AppEvent::HostSystemEq(status)),
                                Err(e) => emit(AppEvent::Error(e)),
                            },
                            None => emit(AppEvent::Error(
                                "System EQ is not available in this build".into(),
                            )),
                        },
                        AppCommand::SystemEqOff => match host.system_eq.as_mut() {
                            Some(f) => match f(SystemEqCommand::Off) {
                                Ok(status) => emit(AppEvent::HostSystemEq(status)),
                                Err(e) => emit(AppEvent::Error(e)),
                            },
                            None => emit(AppEvent::Error(
                                "System EQ is not available in this build".into(),
                            )),
                        },
                        AppCommand::SystemEqStatus => match host.system_eq.as_mut() {
                            Some(f) => match f(SystemEqCommand::Status) {
                                Ok(status) => emit(AppEvent::HostSystemEq(status)),
                                Err(e) => emit(AppEvent::Error(e)),
                            },
                            None => emit(AppEvent::HostSystemEq(SystemEqStatus::default())),
                        },
                    }
                    let _ = &experimental_opt_in; // opt-in lives in the transport policy
                }
            })
            .expect("app core thread spawns");

        AppHandle {
            commands: cmd_tx,
            events: event_rx,
            worker: Some(worker),
        }
    }
}
