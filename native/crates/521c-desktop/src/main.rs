//! 521C desktop application (issue #8).
//!
//! Architecture: a single native process. The Slint UI thread talks to the
//! [`qcy_app::core::AppCore`] worker through typed channels; the worker owns the
//! [`qcy_transport::Transport`] (BlueZ by default, mock when `--mock` is passed)
//! and the [`qcy_host`] services. There is no IPC layer in v1: one process keeps
//! RAM overhead low, removes a whole serialization boundary, and the typed
//! `AppCommand`/`AppEvent` API is already shaped so an IPC boundary can be added
//! later without changing the UI contract (see docs/DESKTOP_ARCHITECTURE.md).
//!
//! Raw GATT bytes stay below the UI boundary. Write authorization is the central
//! `WritePolicy` inside every transport write; this binary never reimplements it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use qcy_app::config::{self, XdgStorage};
use qcy_app::core::{
    AppCommand, AppCore, AppEvent, DeviceSnapshot, HostServices, MediaCommand, MediaRequest,
    SimpleNoise, SystemEqCommand,
};
use qcy_host::codec::{BluezCodecSource, CodecInfo, CodecSource, UnknownCodecSource};
#[cfg(feature = "bluez")]
use qcy_host::game_mode::{GameModeSignal, MprisPresenceSignal};
use qcy_host::mpris::{MediaAction, MprisHost};
use qcy_host::system_eq::{MockSystemEq, PipewireSystemEq, SystemEq, SystemEqStatus};
use qcy_transport::{DiscoveredDevice, Transport, WritePolicy};

slint::include_modules!();

const LOG_LIMIT: usize = 200;

/// Auto Game Mode transition cooldown. Player presence changes are infrequent;
/// 30s suppresses rapid on/off churn when players restart or hand over.
#[cfg(feature = "bluez")]
const AUTO_GAME_COOLDOWN: Duration = Duration::from_secs(30);

fn now_stamp() -> String {
    // Deliberately dependency-free: seconds since UNIX epoch is enough for a
    // session log; wall-clock formatting would pull in a time crate.
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("[+{}s]", d.as_secs()))
        .unwrap_or_default()
}

fn append_log(current: &str, line: &str) -> String {
    let stamped = format!("{} {}", now_stamp(), line);
    let mut lines: Vec<&str> = current.lines().collect();
    lines.push(&stamped);
    if lines.len() > LOG_LIMIT {
        let drop = lines.len() - LOG_LIMIT;
        lines.drain(0..drop);
    }
    lines.join("\n")
}

fn codec_text(info: &CodecInfo) -> String {
    if info.is_unknown() {
        return "unknown (no active A2DP transport exposed by BlueZ)".into();
    }
    let mut parts = Vec::new();
    if let Some(codec) = &info.codec {
        parts.push(codec.clone());
    }
    if let Some(rate) = info.sample_rate_hz {
        parts.push(format!("{rate} Hz"));
    }
    if let Some(profile) = &info.profile {
        parts.push(profile.clone());
    }
    parts.join(" · ")
}

fn system_eq_text(status: &SystemEqStatus) -> String {
    if status.enabled {
        format!(
            "on ({} bands)",
            status.gains.as_ref().map(|g| g.len()).unwrap_or(0)
        )
    } else {
        "off".into()
    }
}

fn apply_snapshot(state: &AppState, snapshot: &DeviceSnapshot) {
    state.set_connected(snapshot.connected);
    state.set_device_name(if snapshot.name.is_empty() {
        "—".into()
    } else {
        snapshot.name.clone().into()
    });
    state.set_device_address(if snapshot.address.is_empty() {
        "—".into()
    } else {
        snapshot.address.clone().into()
    });
    state.set_model_known(snapshot.model_known);
    state.set_rssi(
        snapshot
            .rssi
            .map(|r| format!("{r} dBm"))
            .unwrap_or_else(|| "—".into())
            .into(),
    );
    state.set_firmware(
        snapshot
            .firmware
            .clone()
            .unwrap_or_else(|| "—".into())
            .into(),
    );
    match &snapshot.battery {
        Some(b) => {
            state.set_has_battery(true);
            state.set_battery_left(b.left as i32);
            state.set_battery_right(b.right as i32);
            state.set_battery_case(b.case as i32);
            state.set_charging_left(b.charging_left);
            state.set_charging_right(b.charging_right);
            state.set_charging_case(b.charging_case);
        }
        None => state.set_has_battery(false),
    }
}

fn build_transport(mock: bool) -> Result<Box<dyn Transport + Send>, String> {
    if mock {
        return Ok(Box::new(qcy_transport::mock::MockTransport::new(
            WritePolicy::ht08(),
        )));
    }
    #[cfg(feature = "bluez")]
    {
        let bus = qcy_transport::bluez::ZbusBlueZBus::system()
            .map_err(|e| format!("BlueZ system bus unavailable: {e}"))?;
        Ok(Box::new(qcy_transport::bluez::BlueZTransport::new(
            Box::new(bus),
            WritePolicy::ht08(),
        )))
    }
    #[cfg(not(feature = "bluez"))]
    {
        Err("built without the bluez feature; pass --mock".into())
    }
}

fn build_host_services(mock: bool) -> HostServices {
    let mut host = HostServices::default();

    // MPRIS (session bus). Absent in mock mode or when no session bus exists.
    if !mock {
        #[cfg(feature = "bluez")]
        if let Ok(bus) = qcy_host::mpris::ZbusMprisBus::session() {
            let mpris = MprisHost::new(Box::new(bus));
            host.mpris = Some(Box::new(move |request: MediaRequest| match request {
                MediaRequest::Status => mpris
                    .status(None)
                    .map_err(|e| format!("MPRIS status failed: {e}")),
                MediaRequest::Control(action) => {
                    let action = match action {
                        MediaCommand::Play => MediaAction::Play,
                        MediaCommand::Pause => MediaAction::Pause,
                        MediaCommand::Next => MediaAction::Next,
                        MediaCommand::Previous => MediaAction::Previous,
                    };
                    mpris
                        .control(None, action)
                        .map_err(|e| format!("MPRIS control failed: {e}"))?;
                    mpris
                        .status(None)
                        .map_err(|e| format!("MPRIS status failed: {e}"))
                }
            }));
        }
    }

    // Codec source: BlueZ MediaTransport1 when live, honest unknown otherwise.
    let source: Box<dyn CodecSource + Send> = if mock {
        Box::new(UnknownCodecSource)
    } else {
        #[cfg(feature = "bluez")]
        {
            match qcy_host::codec::ZbusCodecBus::system() {
                Ok(bus) => Box::new(BluezCodecSource::new(Box::new(bus))),
                Err(_) => Box::new(UnknownCodecSource),
            }
        }
        #[cfg(not(feature = "bluez"))]
        Box::new(UnknownCodecSource)
    };
    host.codec = Some(Box::new(move || {
        source.read().unwrap_or_else(|_| CodecInfo::unknown())
    }));

    // System EQ: PipeWire config-dir backend when live, in-memory mock otherwise.
    if mock {
        let mut eq = MockSystemEq::default();
        host.system_eq = Some(Box::new(move |cmd: SystemEqCommand| match cmd {
            SystemEqCommand::On(gains) => eq
                .enable(&gains)
                .map_err(|e| format!("system EQ failed: {e}"))
                .and_then(|()| eq.status().map_err(|e| format!("system EQ failed: {e}"))),
            SystemEqCommand::Off => eq
                .disable()
                .map_err(|e| format!("system EQ failed: {e}"))
                .and_then(|()| eq.status().map_err(|e| format!("system EQ failed: {e}"))),
            SystemEqCommand::Status => eq.status().map_err(|e| format!("system EQ failed: {e}")),
        }));
    } else if let Some(dir) = PipewireSystemEq::default_dir() {
        let mut eq = PipewireSystemEq::new(dir);
        host.system_eq = Some(Box::new(move |cmd: SystemEqCommand| match cmd {
            SystemEqCommand::On(gains) => eq
                .enable(&gains)
                .map_err(|e| format!("system EQ failed: {e}"))
                .and_then(|()| eq.status().map_err(|e| format!("system EQ failed: {e}"))),
            SystemEqCommand::Off => eq
                .disable()
                .map_err(|e| format!("system EQ failed: {e}"))
                .and_then(|()| eq.status().map_err(|e| format!("system EQ failed: {e}"))),
            SystemEqCommand::Status => eq.status().map_err(|e| format!("system EQ failed: {e}")),
        }));
    }

    host
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mock = args.iter().any(|a| a == "--mock") || cfg!(not(feature = "bluez"));
    let self_test = args.iter().any(|a| a == "--self-test");
    let close_self_test = args.iter().any(|a| a == "--close-self-test");

    // XDG config: load before the UI so persisted preferences apply at startup.
    let mut storage = XdgStorage::default_path()
        .map(XdgStorage::new)
        .expect("XDG config path resolvable");
    let loaded = config::load_persisted_config(&mut storage);
    if loaded.used_defaults && !loaded.errors.is_empty() {
        eprintln!(
            "521c: stored config rejected ({}), using defaults: {}",
            loaded.errors.len(),
            config::summarize_errors(&loaded.errors, 3)
        );
    }

    let transport = match build_transport(mock) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("521c: falling back to mock transport: {e}");
            Box::new(qcy_transport::mock::MockTransport::new(WritePolicy::ht08()))
        }
    };
    let host = build_host_services(mock);
    let mut handle = AppCore::start(transport, host, loaded.config.known_devices.clone());

    // Auto-attach: earbuds already connected to the PC (e.g. paired for audio
    // before the app started) are detected and attached without a manual
    // scan/connect. No-op when nothing is connected; scan stays available.
    if !mock {
        let _ = handle.commands.send(AppCommand::AttachConnected);
    }

    // Auto Game Mode (issue #13 wiring, issue #8): MPRIS player presence drives
    // the earbuds' game mode through the same typed command path as the UI. Off by
    // default (config `autoGame`), event-driven, and writes only happen while
    // connected — never any BLE traffic while idle.
    let connected_flag: Arc<AtomicBool> = Arc::default();
    let auto_game_desired: Arc<AtomicBool> = Arc::default();
    let auto_game_enabled = !mock && loaded.config.external.auto_game;
    #[cfg(feature = "bluez")]
    if auto_game_enabled {
        let keyword = loaded.config.external.auto_game_keyword.clone();
        let sender = handle.commands.clone();
        let connected = Arc::clone(&connected_flag);
        let desired = Arc::clone(&auto_game_desired);
        std::thread::Builder::new()
            .name("521c-auto-game".into())
            .spawn(move || {
                let Ok(mut signal) = MprisPresenceSignal::session() else {
                    eprintln!("521c: auto game mode unavailable (no D-Bus session bus)");
                    return;
                };
                let mut controller = qcy_host::game_mode::GameModeController::new(
                    qcy_host::game_mode::GameModeRule::new(vec![keyword]),
                    AUTO_GAME_COOLDOWN,
                );
                while let Some(event) = signal.next_event() {
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    let decision = controller.handle(event, now_ms);
                    desired.store(controller.is_on(), Ordering::SeqCst);
                    if decision.changed && connected.load(Ordering::SeqCst) {
                        let _ = sender.send(AppCommand::SetGameMode(decision.game_mode_on));
                    }
                }
            })
            .expect("auto game thread spawns");
    }

    let app = MainWindow::new().expect("Slint window creates");
    let state = app.global::<AppState>();
    state.set_mock_mode(mock);
    state.set_transport_badge(if mock { "mock".into() } else { "bluez".into() });
    state.set_status_line(
        if mock {
            "MOCK transport: deterministic development backend, not real hardware."
        } else {
            "BlueZ transport: checking for already-connected earbuds (scan to find others)."
        }
        .into(),
    );

    // Discovered devices shared between the event pump and UI callbacks.
    let devices: Arc<Mutex<Vec<DiscoveredDevice>>> = Arc::default();

    // ---- UI callbacks -> typed commands ------------------------------------
    let sender = handle.commands.clone();
    app.on_scan_clicked(move || {
        let _ = sender.send(AppCommand::Scan);
    });
    {
        let devices = Arc::clone(&devices);
        let sender = handle.commands.clone();
        app.on_connect_clicked(move |index| {
            let list = devices.lock().expect("devices mutex");
            if let Some(dev) = list.get(index as usize) {
                let _ = sender.send(AppCommand::Connect(dev.address.clone()));
            }
        });
    }
    {
        let sender = handle.commands.clone();
        app.on_disconnect_clicked(move || {
            let _ = sender.send(AppCommand::Disconnect);
        });
    }
    {
        let sender = handle.commands.clone();
        app.on_refresh_clicked(move || {
            let _ = sender.send(AppCommand::RefreshStatus);
        });
    }
    {
        let sender = handle.commands.clone();
        let weak = app.as_weak();
        app.on_confirm_model_clicked(move || {
            // The address comes from the connected snapshot shown in the UI;
            // the core still refuses confirmations for anything but the
            // currently connected device.
            let Some(app) = weak.upgrade() else {
                return;
            };
            let address = app.global::<AppState>().get_device_address().to_string();
            if address.is_empty() || address == "—" {
                return;
            }
            let _ = sender.send(AppCommand::ConfirmModel { address });
        });
    }
    {
        let sender = handle.commands.clone();
        app.on_noise_changed(move |mode| {
            let noise = match mode {
                1 => SimpleNoise::Anc,
                2 => SimpleNoise::Outdoor,
                3 => SimpleNoise::Transparency,
                _ => SimpleNoise::Off,
            };
            let _ = sender.send(AppCommand::SetNoise(noise));
        });
    }
    {
        let sender = handle.commands.clone();
        app.on_game_mode_changed(move |on| {
            let _ = sender.send(AppCommand::SetGameMode(on));
        });
    }
    {
        let sender = handle.commands.clone();
        app.on_sleep_mode_changed(move |on| {
            let _ = sender.send(AppCommand::SetSleepMode(on));
        });
    }
    {
        let sender = handle.commands.clone();
        app.on_in_ear_changed(move |on| {
            let _ = sender.send(AppCommand::SetInEarDetection(on));
        });
    }
    {
        let sender = handle.commands.clone();
        app.on_find_confirmed(move |confirmed| {
            let _ = sender.send(AppCommand::FindChime {
                led: true,
                chime: true,
                tone_id: 0x01,
                confirmed_not_worn: confirmed,
            });
        });
    }
    {
        let sender = handle.commands.clone();
        app.on_media_play(move || {
            let _ = sender.send(AppCommand::MediaControl(MediaCommand::Play));
        });
    }
    {
        let sender = handle.commands.clone();
        app.on_media_pause(move || {
            let _ = sender.send(AppCommand::MediaControl(MediaCommand::Pause));
        });
    }
    {
        let sender = handle.commands.clone();
        app.on_media_next(move || {
            let _ = sender.send(AppCommand::MediaControl(MediaCommand::Next));
        });
    }
    {
        let sender = handle.commands.clone();
        app.on_media_previous(move || {
            let _ = sender.send(AppCommand::MediaControl(MediaCommand::Previous));
        });
    }
    {
        let sender = handle.commands.clone();
        app.on_media_refresh(move || {
            let _ = sender.send(AppCommand::MediaStatus);
        });
    }
    {
        let sender = handle.commands.clone();
        app.on_codec_refresh(move || {
            let _ = sender.send(AppCommand::CodecStatus);
        });
    }
    {
        let sender = handle.commands.clone();
        app.on_system_eq_toggled(move |on| {
            let cmd = if on {
                AppCommand::SystemEqOn(vec![0.0; 10])
            } else {
                AppCommand::SystemEqOff
            };
            let _ = sender.send(cmd);
        });
    }

    // Shared config: the event pump persists model attestations immediately
    // (crash-safe); the exit handler saves the final state.
    let shared_config = Arc::new(Mutex::new(loaded.config.clone()));

    // ---- event pump: typed events -> UI state ------------------------------
    let events = handle.take_events().expect("event receiver available");
    let devices_pump = Arc::clone(&devices);
    let connected_pump = Arc::clone(&connected_flag);
    let desired_pump = Arc::clone(&auto_game_desired);
    let config_pump = Arc::clone(&shared_config);
    let pump_commands = handle.commands.clone();
    let weak = app.as_weak();
    std::thread::Builder::new()
        .name("521c-event-pump".into())
        .spawn(move || {
            while let Ok(event) = events.recv() {
                let devices = Arc::clone(&devices_pump);
                let connected_flag = Arc::clone(&connected_pump);
                let config = Arc::clone(&config_pump);
                let auto_game_desired = Arc::clone(&desired_pump);
                let sender = pump_commands.clone();
                let weak = weak.clone();
                let ok = slint::invoke_from_event_loop(move || {
                    let app = match weak.upgrade() {
                        Some(app) => app,
                        None => return,
                    };
                    let state = app.global::<AppState>();
                    match event {
                        AppEvent::Discovered(list) => {
                            let labels: Vec<slint::SharedString> = list
                                .iter()
                                .map(|d| {
                                    format!(
                                        "{} ({})",
                                        if d.name.is_empty() {
                                            "unknown"
                                        } else {
                                            &d.name
                                        },
                                        d.address
                                    )
                                    .into()
                                })
                                .collect();
                            *devices.lock().expect("devices mutex") = list;
                            app.set_devices(slint::ModelRc::new(slint::VecModel::from_slice(
                                &labels,
                            )));
                            state.set_status_line(
                                format!("{} device(s) found.", labels.len()).into(),
                            );
                        }
                        AppEvent::StateChanged(snapshot) => {
                            apply_snapshot(&state, &snapshot);
                            connected_flag.store(snapshot.connected, Ordering::SeqCst);
                            // Reconcile Auto Game Mode on (re)connect: if a matching
                            // player is present, bring game mode on. Desired-off never
                            // forces the device, so manual settings survive reconnects.
                            if snapshot.connected
                                && auto_game_enabled
                                && auto_game_desired.load(Ordering::SeqCst)
                            {
                                let _ = sender.send(AppCommand::SetGameMode(true));
                            }
                            state.set_status_line(
                                (if snapshot.connected {
                                    "Connected."
                                } else {
                                    "Disconnected."
                                })
                                .into(),
                            );
                        }
                        AppEvent::HostMedia(media) => {
                            state.set_media_player(media.player.into());
                            state.set_media_title(if media.title.is_empty() {
                                "—".into()
                            } else {
                                media.title.into()
                            });
                            state.set_media_artist(if media.artist.is_empty() {
                                "—".into()
                            } else {
                                media.artist.into()
                            });
                            state.set_media_playing(media.playing);
                        }
                        AppEvent::HostCodec(info) => {
                            state.set_codec_text(codec_text(&info).into());
                        }
                        AppEvent::HostSystemEq(status) => {
                            state.set_system_eq_on(status.enabled);
                            state.set_system_eq_text(system_eq_text(&status).into());
                        }
                        AppEvent::Error(message) => {
                            state.set_status_line(message.clone().into());
                            let log = state.get_event_log();
                            state.set_event_log(
                                append_log(&log, &format!("ERROR: {message}")).into(),
                            );
                        }
                        AppEvent::Denied(message) => {
                            state.set_status_line(format!("Denied: {message}").into());
                            let log = state.get_event_log();
                            state.set_event_log(
                                append_log(&log, &format!("DENIED: {message}")).into(),
                            );
                        }
                        AppEvent::Info(message) => {
                            state.set_status_line(message.clone().into());
                            let log = state.get_event_log();
                            state.set_event_log(append_log(&log, &message).into());
                        }
                        AppEvent::ModelConfirmed { address } => {
                            // Persist the attestation immediately (local-only
                            // field) so it survives crashes, not just clean exit.
                            {
                                let mut cfg = config.lock().expect("config mutex");
                                if !cfg.known_devices.contains(&address) {
                                    cfg.known_devices.push(address.clone());
                                }
                                let mut storage = XdgStorage::default_path()
                                    .map(XdgStorage::new)
                                    .expect("XDG config path resolvable");
                                config::save_persisted_config(&mut storage, &cfg);
                            }
                            state.set_status_line(
                                format!("Model confirmed for {address}; controls enabled.").into(),
                            );
                            let log = state.get_event_log();
                            state.set_event_log(
                                append_log(&log, &format!("Model confirmed: {address}")).into(),
                            );
                        }
                    }
                });
                if ok.is_err() {
                    break; // UI event loop ended
                }
            }
        })
        .expect("event pump thread spawns");

    // Initial host status so panels are honest from the first frame.
    let _ = handle.send(AppCommand::CodecStatus);
    let _ = handle.send(AppCommand::SystemEqStatus);

    if self_test {
        // Launch verification: process events briefly, then exit cleanly. Used by
        // the packaging gate where interactive display time is not available.
        std::thread::sleep(Duration::from_millis(500));
        println!(
            "521c: self-test OK (transport={}, window created, event pump alive)",
            if mock { "mock" } else { "bluez" }
        );
        return;
    }

    let config_for_exit = Arc::clone(&shared_config);
    app.window().on_close_requested(move || {
        // Persist the full local config (portable + local-only) on clean exit.
        {
            let cfg = config_for_exit.lock().expect("config mutex");
            let mut storage = XdgStorage::default_path()
                .map(XdgStorage::new)
                .expect("XDG config path resolvable");
            config::save_persisted_config(&mut storage, &cfg);
        }
        // v1 is a normal windowed app with no tray/background mode (see
        // DESKTOP_ARCHITECTURE.md §6): a normal close must end the event loop
        // so the process exits and the core worker shuts down. HideWindow
        // alone would leave an invisible process running (issue #40).
        let _ = slint::quit_event_loop();
        slint::CloseRequestResponse::HideWindow
    });

    // Deterministic close-lifecycle gate (issue #40): exercise the exact
    // close path a window manager triggers (WindowEvent::CloseRequested) and
    // require the event loop to exit. A hang here means a normal close would
    // leave an invisible process behind.
    let _close_test_timer = close_self_test.then(|| {
        let weak = app.as_weak();
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(500),
            move || {
                if let Some(app) = weak.upgrade() {
                    let _ = app
                        .window()
                        .try_dispatch_event(slint::platform::WindowEvent::CloseRequested);
                }
            },
        );
        timer
    });

    app.run().expect("Slint event loop runs");

    if close_self_test {
        // The event loop returned after the synthetic close: the process is
        // about to exit cleanly. Verify the config persisted on the close
        // path loads back as valid.
        let mut storage = XdgStorage::default_path()
            .map(XdgStorage::new)
            .expect("XDG config path resolvable");
        let reloaded = config::load_persisted_config(&mut storage);
        if reloaded.errors.is_empty() {
            println!(
                "521c: close-self-test OK (close ended the event loop; persisted config valid)"
            );
        } else {
            eprintln!(
                "521c: close-self-test FAIL (persisted config invalid: {})",
                config::summarize_errors(&reloaded.errors, 3)
            );
            std::process::exit(1);
        }
    }
}
