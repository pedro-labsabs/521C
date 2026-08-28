//! 521cctl — CLI with the same command surface as the GUI core.
//!
//! Mock transport is the deliberate default (tests/dev, no hardware needed). Pass
//! `--bluez` to operate a real, explicitly selected device through the system BlueZ
//! stack. Outbound writes pass through the central write-authorization policy either way.
//!
//! Argument handling (#70): flags are accepted anywhere on the command line
//! (before or after the command word); unknown flags, missing flag values and
//! unexpected extra arguments are hard errors. Write commands (`anc`,
//! `game-mode`) require an explicit mode — there is no silent default.

use qcy_protocol::packet::encode_command;
use qcy_protocol::Cmd;
use qcy_transport::policy::{WritePolicy, CHAR_COMMAND_WRITE};
use qcy_transport::{mock::MockTransport, DiscoveredDevice, Transport, TransportError};

#[cfg(feature = "host")]
use qcy_host::mpris::{MediaAction, MprisHost};
#[cfg(feature = "host")]
use qcy_host::system_eq::{PipewireSystemEq, SystemEq};

const CHAR_BATTERY: &str = "00000008-0000-1000-8000-00805f9b34fb";
const CHAR_VERSION: &str = "00000007-0000-1000-8000-00805f9b34fb";

#[derive(Debug, PartialEq)]
struct Options {
    bluez: bool,
    spp: bool,
    adapter: String,
    device: Option<String>,
    /// RFCOMM channel override for `--spp` (default 1).
    channel: Option<u8>,
    /// Explicit user attestation that the selected device's model is known.
    /// Required for writes over `--spp`, where no advertised name can prove
    /// the model. Never persisted; applies to this invocation only.
    attest: bool,
    help: bool,
}

/// Take the value of a flag that requires one (#70): missing values and
/// values that look like the next flag are hard errors, never silent defaults.
fn flag_value<I: Iterator<Item = String>>(args: &mut I, flag: &str) -> Result<String, String> {
    let value = args
        .next()
        .ok_or_else(|| format!("{flag} requires a value (see `521cctl help`)"))?;
    if value.starts_with("--") {
        return Err(format!(
            "{flag} requires a value; got another flag (`{value}`)"
        ));
    }
    Ok(value)
}

/// Parse CLI options (#70). Flags are accepted anywhere on the command line —
/// `521cctl anc off --bluez` behaves exactly like `521cctl --bluez anc off` —
/// so a backend flag placed after the command word is honored instead of being
/// silently treated as a positional argument. Unknown flags, missing flag
/// values and unparseable `--channel` values are hard errors. Everything that
/// is not a flag is returned as a positional argument, in order.
fn parse_opts<I: Iterator<Item = String>>(args: I) -> Result<(Options, Vec<String>), String> {
    let mut opts = Options {
        bluez: false,
        spp: false,
        adapter: "hci0".to_string(),
        device: None,
        channel: None,
        attest: false,
        help: false,
    };
    let mut rest = Vec::new();
    let mut args = args;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--bluez" => opts.bluez = true,
            "--spp" => opts.spp = true,
            // --mock clears an earlier backend flag; the last backend flag on
            // the line wins (documented in help).
            "--mock" => {
                opts.bluez = false;
                opts.spp = false;
            }
            "--adapter" => opts.adapter = flag_value(&mut args, "--adapter")?,
            "--device" => opts.device = Some(flag_value(&mut args, "--device")?),
            "--channel" => {
                let raw = flag_value(&mut args, "--channel")?;
                opts.channel =
                    Some(raw.parse::<u8>().map_err(|_| {
                        format!("--channel: not a number: `{raw}` (expected 1-30)")
                    })?);
            }
            "--attest" => opts.attest = true,
            "help" | "-h" | "--help" => opts.help = true,
            other if other.starts_with('-') => {
                return Err(format!("unknown flag: `{other}` (see `521cctl help`)"))
            }
            _ => rest.push(a),
        }
    }
    Ok((opts, rest))
}

/// Reject unexpected positional arguments after a command's known ones (#70):
/// typos must fail loudly, not run against the wrong target.
fn expect_no_more_args(cmd: &str, extra: &[String]) -> Result<(), String> {
    if let Some(first) = extra.first() {
        return Err(format!(
            "unexpected argument for `{cmd}`: `{first}` (see `521cctl help`)"
        ));
    }
    Ok(())
}

fn build_transport(opts: &Options) -> Result<Box<dyn Transport>, TransportError> {
    if opts.spp {
        build_spp(opts)
    } else if opts.bluez {
        build_bluez(opts)
    } else {
        Ok(Box::new(MockTransport::new(WritePolicy::ht08())))
    }
}

/// SPP/RFCOMM backend (issue #50): the control path BlueZ actually exposes for
/// QCY dual-mode earbuds on Linux. Raw `AF_BLUETOOTH` socket, no root needed.
fn build_spp(opts: &Options) -> Result<Box<dyn Transport>, TransportError> {
    let mut t = qcy_transport::rfcomm::RfcommTransport::new(
        Box::new(qcy_transport::rfcomm::RawRfcommSocketFactory::default()),
        WritePolicy::ht08(),
    );
    if let Some(ch) = opts.channel {
        if !(1..=30).contains(&ch) {
            return Err(TransportError::InvalidArgument(format!(
                "RFCOMM channel out of range: {ch}"
            )));
        }
        t = t.with_channel(ch);
    }
    Ok(Box::new(t))
}

#[cfg(feature = "bluez")]
fn build_bluez(opts: &Options) -> Result<Box<dyn Transport>, TransportError> {
    let bus = qcy_transport::bluez::ZbusBlueZBus::system()?;
    let t = qcy_transport::bluez::BlueZTransport::new(Box::new(bus), WritePolicy::ht08())
        .with_adapter(opts.adapter.clone());
    Ok(Box::new(t))
}

#[cfg(not(feature = "bluez"))]
fn build_bluez(_opts: &Options) -> Result<Box<dyn Transport>, TransportError> {
    Err(TransportError::Bus(
        "built without the `bluez` feature; recompile with it enabled".into(),
    ))
}

fn pick_device(
    list: &[DiscoveredDevice],
    wanted: Option<&str>,
) -> Result<DiscoveredDevice, TransportError> {
    if let Some(w) = wanted {
        list.iter()
            .find(|d| d.address.eq_ignore_ascii_case(w) || d.name == w)
            .cloned()
            .ok_or_else(|| TransportError::NotFound(w.to_string()))
    } else {
        list.first()
            .cloned()
            .ok_or_else(|| TransportError::NotFound("no QCY device discovered".into()))
    }
}

/// Resolve the target device. SPP devices do not advertise (issue #50): the
/// earbuds are already paired at the host level, so when the transport has no
/// scan surface an explicit address is the selection. Model truth is preserved
/// either way: the device stays read-only until proven or attested.
fn resolve_device(
    t: &mut Box<dyn Transport>,
    wanted: Option<&str>,
) -> Result<DiscoveredDevice, TransportError> {
    let list = t.scan()?;
    if !list.is_empty() {
        return pick_device(&list, wanted);
    }
    let addr = wanted.ok_or_else(|| {
        TransportError::NotFound("no QCY device discovered; pass --device <addr>".into())
    })?;
    Ok(DiscoveredDevice {
        address: qcy_transport::normalize_address(addr),
        name: "paired device (no advertisement)".to_string(),
        rssi: None,
        model_known: false,
    })
}

/// Connect and apply the per-invocation model attestation, if requested.
fn attach(
    t: &mut Box<dyn Transport>,
    opts: &Options,
    dev: &DiscoveredDevice,
) -> Result<(), TransportError> {
    t.connect(&dev.address)?;
    if opts.attest {
        t.attest_model_known();
    }
    Ok(())
}

fn battery_label(bytes: &[u8]) -> String {
    let cell = |b: Option<&u8>| match b {
        Some(v) => format!("{}%", v & 0x7f),
        None => "--".to_string(),
    };
    format!(
        "L {}  R {}  case {}",
        cell(bytes.first()),
        cell(bytes.get(1)),
        cell(bytes.get(2))
    )
}

fn firmware_label(bytes: &[u8]) -> String {
    if bytes.len() >= 3 {
        format!("{}.{}.{}", bytes[0], bytes[1], bytes[2])
    } else {
        "--".to_string()
    }
}

/// Hardware-validated HT08 ANC scene table (live evidence, #50/#52/#54):
/// opcode 0x17 `AncSetting` payloads `[mode, subScene, noiseValue]`. This is
/// the same table as `qcy_app::core::SimpleNoise::scene` in the desktop core —
/// the falsified 0x0C `NoiseCancelMode` is never written by the CLI (#59).
/// Returns the canonical mode label and the scene payload.
fn anc_scene(mode: &str) -> Option<(&'static str, [u8; 3])> {
    let scene: [u8; 3] = match mode {
        "off" => [0x02, 0x00, 0x00],
        // "on" keeps the old CLI spelling working; it maps to the validated
        // indoor ANC scene (the default ANC mode).
        "on" | "anc" | "indoor" => [0x01, 0x01, 0x02],
        "commuting" => [0x01, 0x02, 0x02],
        "noisy" => [0x01, 0x03, 0x02],
        "wind" => [0x01, 0x04, 0x02],
        "adaptive" => [0x01, 0x05, 0x02],
        "transparency" => [0x03, 0x02, 0x04],
        _ => return None,
    };
    let label: &'static str = match mode {
        "on" | "indoor" | "anc" => "anc",
        "off" => "off",
        "commuting" => "commuting",
        "noisy" => "noisy",
        "wind" => "wind",
        "adaptive" => "adaptive",
        "transparency" => "transparency",
        _ => return None,
    };
    Some((label, scene))
}

const ANC_MODES: &str =
    "off | on | anc | indoor | commuting | noisy | wind | adaptive | transparency";

fn print_help() {
    println!("521cctl [--mock|--bluez|--spp] [--adapter hci0] [--device <addr>] [--channel n] [--attest] <command>");
    println!("Flags are accepted before or after the command word; unknown flags are errors.");
    println!("  scan                     list candidate QCY devices");
    println!("  connect [<addr>]         connect and resolve characteristics");
    println!("  status                   connect + battery/firmware readout");
    println!("  battery                  connect + battery readout");
    println!("  anc <{ANC_MODES}>");
    println!("                           validated 0x17 AncSetting scenes");
    println!(
        "                           (0x0C NoiseCancelMode is falsified on HT08 and never written)"
    );
    println!("  game-mode <on|off>       low-latency mode (explicit value required)");
    println!("Host services (never written to the earbuds):");
    println!("  media <status|play|pause|next|prev>   MPRIS media control");
    println!(
        "  codec                                 host codec/sample-rate (unknown if unavailable)"
    );
    println!("  system-eq <on|off|status> [gains...]  PipeWire System EQ");
    println!("Mock is the deliberate default; --bluez targets BLE GATT and --spp targets");
    println!("SPP/RFCOMM (the control path BlueZ exposes for QCY earbuds on Linux, issue #50).");
    println!("--attest is explicit user attestation that the device model is known (writes).");
    println!("Independent. Not affiliated with QCY.");
}

fn run() -> Result<(), TransportError> {
    run_args(std::env::args().skip(1).collect::<Vec<String>>())
}

fn run_args<I: IntoIterator<Item = String>>(args: I) -> Result<(), TransportError> {
    let usage = |message: String| TransportError::InvalidArgument(message);
    let (opts, rest) = parse_opts(args.into_iter()).map_err(usage)?;
    let cmd = rest.first().cloned().unwrap_or_else(|| "help".into());
    let backend = if opts.spp {
        "spp"
    } else if opts.bluez {
        "bluez"
    } else {
        "mock"
    };

    if opts.help {
        print_help();
        return Ok(());
    }
    match cmd.as_str() {
        "help" => print_help(),
        "scan" => {
            expect_no_more_args("scan", &rest[1..]).map_err(usage)?;
            let mut t = build_transport(&opts)?;
            let list = t.scan()?;
            if list.is_empty() {
                println!("no QCY devices discovered ({backend})");
            }
            for d in &list {
                let model = if d.model_known {
                    "HT08"
                } else {
                    "unknown-model (read-only)"
                };
                let rssi = d
                    .rssi
                    .map(|r| format!("{r} dBm"))
                    .unwrap_or_else(|| "?".into());
                println!("{}  {}  {}  {rssi}", d.address, d.name, model);
            }
        }
        "connect" => {
            expect_no_more_args("connect", &rest[2..]).map_err(usage)?;
            let mut t = build_transport(&opts)?;
            let dev = resolve_device(
                &mut t,
                rest.get(1).map(|s| s.as_str()).or(opts.device.as_deref()),
            )?;
            attach(&mut t, &opts, &dev)?;
            println!("connected to {} ({}) via {backend}", dev.name, dev.address);
        }
        "status" | "battery" => {
            expect_no_more_args(&cmd, &rest[1..]).map_err(usage)?;
            let mut t = build_transport(&opts)?;
            let dev = resolve_device(&mut t, opts.device.as_deref())?;
            if !dev.model_known && !opts.attest {
                println!("note: model not proven; treating as read-only");
            }
            attach(&mut t, &opts, &dev)?;
            let battery = t.read(CHAR_BATTERY)?;
            println!("{}  {}  {}", dev.name, dev.address, backend);
            println!("battery: {}", battery_label(&battery));
            if cmd == "status" {
                match t.read(CHAR_VERSION) {
                    Ok(fw) => println!("firmware: {}", firmware_label(&fw)),
                    Err(e) => println!("firmware: unavailable ({e})"),
                }
            }
        }
        "anc" => {
            // Validated 0x17 AncSetting scenes (#59). A bare `anc` no longer
            // defaults to a write (#70): the mode must be explicit.
            let mode = rest
                .get(1)
                .ok_or_else(|| usage(format!("anc requires an explicit mode: {ANC_MODES}")))?;
            expect_no_more_args("anc", &rest[2..]).map_err(usage)?;
            let (label, [m, sub, noise]) = anc_scene(mode).ok_or_else(|| {
                usage(format!("unknown anc mode: `{mode}`; expected {ANC_MODES}"))
            })?;
            let mut t = build_transport(&opts)?;
            let dev = resolve_device(&mut t, opts.device.as_deref())?;
            attach(&mut t, &opts, &dev)?;
            let frame = encode_command(Cmd::AncSetting as u8, &[m, sub, noise])
                .map_err(|e| TransportError::InvalidArgument(format!("{e:?}")))?;
            t.write(&frame)?;
            println!("anc {label} -> 0x17 AncSetting [{m}, {sub}, {noise}]");
        }
        "game-mode" => {
            // A bare `game-mode` no longer defaults to on (#70): write
            // commands require explicit intent.
            let mode = rest
                .get(1)
                .ok_or_else(|| usage("game-mode requires an explicit value: on | off".into()))?;
            expect_no_more_args("game-mode", &rest[2..]).map_err(usage)?;
            let on = match mode.as_str() {
                "on" => true,
                "off" => false,
                other => {
                    return Err(usage(format!(
                        "unknown game-mode value: `{other}`; expected on | off"
                    )))
                }
            };
            let mut t = build_transport(&opts)?;
            let dev = resolve_device(&mut t, opts.device.as_deref())?;
            attach(&mut t, &opts, &dev)?;
            let frame = encode_command(Cmd::LowLatency as u8, &[if on { 0x01 } else { 0x02 }])
                .map_err(|e| TransportError::InvalidArgument(format!("{e:?}")))?;
            t.write(&frame)?;
            println!("game-mode {}", if on { "on" } else { "off" });
        }
        "media" => {
            let sub = rest.get(1).map(|s| s.as_str()).unwrap_or("status");
            expect_no_more_args("media", &rest[2..]).map_err(usage)?;
            media_cmd(sub)?;
        }
        "codec" => {
            expect_no_more_args("codec", &rest[1..]).map_err(usage)?;
            codec_cmd();
        }
        "system-eq" => {
            let sub = rest.get(1).map(|s| s.as_str()).unwrap_or("status");
            // Unparseable gains are a hard error, not silently dropped (#70).
            let mut gains: Vec<f64> = Vec::new();
            for raw in &rest[2..] {
                let gain: f64 = raw.parse().map_err(|_| {
                    usage(format!(
                        "system-eq: not a number: `{raw}` (gains are in dB)"
                    ))
                })?;
                gains.push(gain);
            }
            system_eq_cmd(sub, &gains)?;
        }
        other => {
            return Err(usage(format!(
                "unknown command: `{other}` (see `521cctl help`)"
            )));
        }
    }
    // Keep the command characteristic referenced so the allowlist is exercised in builds.
    let _ = CHAR_COMMAND_WRITE;
    Ok(())
}

/* ------------------------------------------------------------------ */
/* Host-service commands (issue #13): MPRIS media, codec, System EQ.    */
/* These are host features — they never write to the earbuds.           */
/* ------------------------------------------------------------------ */

#[cfg(feature = "host")]
fn media_cmd(sub: &str) -> Result<(), TransportError> {
    let bus =
        qcy_host::mpris::ZbusMprisBus::session().map_err(|e| TransportError::Bus(e.to_string()))?;
    let host = MprisHost::new(Box::new(bus));
    let map = |e: qcy_host::HostError| TransportError::Bus(e.to_string());
    match sub {
        "status" => {
            let st = host.status(None).map_err(map)?;
            if st.player.is_empty() {
                println!("no MPRIS player available");
            } else {
                let vol = st.volume.map(|v| format!("  vol {v}%")).unwrap_or_default();
                println!(
                    "{}  [{}]  {} — {}{vol}",
                    st.player,
                    if st.playing { "playing" } else { "paused" },
                    st.title,
                    st.artist
                );
            }
        }
        "play" => host.control(None, MediaAction::Play).map_err(map)?,
        "pause" => host.control(None, MediaAction::Pause).map_err(map)?,
        "next" => host.control(None, MediaAction::Next).map_err(map)?,
        "prev" | "previous" => host.control(None, MediaAction::Previous).map_err(map)?,
        other => {
            return Err(TransportError::InvalidArgument(format!(
                "unknown media subcommand: {other}"
            )))
        }
    }
    Ok(())
}

#[cfg(not(feature = "host"))]
fn media_cmd(_sub: &str) -> Result<(), TransportError> {
    Err(TransportError::Bus(
        "built without the `host` feature; recompile with it enabled".into(),
    ))
}

#[cfg(feature = "host")]
fn codec_cmd() {
    use qcy_host::codec::{BluezCodecSource, CodecSource, ZbusCodecBus};
    // Codec facts live in the host Bluetooth/audio stack (issue #13). Read them
    // passively from BlueZ MediaTransport1 objects; when the stack is unreachable
    // or exposes nothing, report unknown — never invent a value.
    let (info, note) = match ZbusCodecBus::system() {
        Ok(bus) => (
            BluezCodecSource::new(Box::new(bus))
                .read()
                .unwrap_or_default(),
            None,
        ),
        Err(e) => (
            qcy_host::codec::CodecInfo::unknown(),
            Some(format!("({e})")),
        ),
    };
    println!(
        "codec:        {}",
        info.codec.as_deref().unwrap_or("unknown")
    );
    println!(
        "sample rate:  {}",
        info.sample_rate_hz
            .map(|r| format!("{r} Hz"))
            .unwrap_or_else(|| "unknown".into())
    );
    println!(
        "profile:      {}",
        info.profile.as_deref().unwrap_or("unknown")
    );
    if let Some(note) = note {
        println!("note:         BlueZ not reachable {note}; fields reported as unknown");
    } else if info.is_unknown() {
        println!("note:         no A2DP transport reported by BlueZ (nothing streaming?)");
    }
}

#[cfg(not(feature = "host"))]
fn codec_cmd() {
    println!("codec:        unknown");
    println!("sample rate:  unknown");
    println!("profile:      unknown");
    println!("note:         built without the `host` feature");
}

#[cfg(feature = "host")]
fn system_eq_cmd(sub: &str, gains: &[f64]) -> Result<(), TransportError> {
    let dir = PipewireSystemEq::default_dir().ok_or_else(|| {
        TransportError::Bus("cannot determine HOME for the PipeWire config dir".into())
    })?;
    let mut eq = PipewireSystemEq::new(dir);
    let map = |e: qcy_host::HostError| TransportError::Bus(e.to_string());
    match sub {
        "on" | "enable" => {
            let gains: Vec<f64> = if gains.is_empty() {
                vec![0.0; qcy_host::system_eq::EQ_BAND_COUNT]
            } else {
                gains.to_vec()
            };
            eq.enable(&gains).map_err(map)?;
            println!("system-eq enabled ({} bands)", gains.len());
            println!("note: applying requires PipeWire to load the artifact, e.g. `systemctl --user restart filter-chain.service` (see docs/DEVELOPMENT.md, Host services).");
        }
        "off" | "disable" => {
            eq.disable().map_err(map)?;
            println!("system-eq disabled (config artifact removed)");
        }
        "status" => {
            let st = eq.status().map_err(map)?;
            println!("system-eq {}", if st.enabled { "on" } else { "off" });
        }
        other => {
            return Err(TransportError::InvalidArgument(format!(
                "unknown system-eq subcommand: {other}"
            )))
        }
    }
    Ok(())
}

#[cfg(not(feature = "host"))]
fn system_eq_cmd(_sub: &str, _gains: &[f64]) -> Result<(), TransportError> {
    Err(TransportError::Bus(
        "built without the `host` feature; recompile with it enabled".into(),
    ))
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

/* ------------------------------------------------------------------ */
/* Tests (#59 CLI remap, #70 argument handling)                        */
/* ------------------------------------------------------------------ */

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    /* #70: flags are accepted anywhere on the command line */

    #[test]
    fn flags_before_the_command_word_are_parsed() {
        let (opts, rest) =
            parse_opts(args(&["--bluez", "--device", "AA:BB", "anc", "off"]).into_iter())
                .expect("parse succeeds");
        assert!(opts.bluez);
        assert_eq!(opts.device.as_deref(), Some("AA:BB"));
        assert_eq!(rest, vec!["anc", "off"]);
    }

    #[test]
    fn flags_after_the_command_word_are_honored_not_ignored() {
        // The audit case: `521cctl anc off --bluez` used to parse `--bluez`
        // as a positional argument and silently run on mock.
        let (opts, rest) =
            parse_opts(args(&["anc", "off", "--bluez"]).into_iter()).expect("parse succeeds");
        assert!(
            opts.bluez,
            "--bluez after the command word must select the BlueZ backend"
        );
        assert_eq!(rest, vec!["anc", "off"]);
    }

    #[test]
    fn flags_interleaved_with_positionals_are_honored() {
        let (opts, rest) = parse_opts(
            args(&["status", "--adapter", "hci1", "--attest", "--channel", "7"]).into_iter(),
        )
        .expect("parse succeeds");
        assert_eq!(opts.adapter, "hci1");
        assert!(opts.attest);
        assert_eq!(opts.channel, Some(7));
        assert_eq!(rest, vec!["status"]);
    }

    #[test]
    fn last_backend_flag_wins() {
        let (opts, _) =
            parse_opts(args(&["--bluez", "--mock", "scan"]).into_iter()).expect("parse succeeds");
        assert!(!opts.bluez);
        let (opts, _) =
            parse_opts(args(&["--mock", "--bluez", "scan"]).into_iter()).expect("parse succeeds");
        assert!(opts.bluez);
    }

    /* #70: unknown or malformed arguments are hard errors */

    #[test]
    fn unknown_flag_is_a_clear_error() {
        let err = parse_opts(args(&["anc", "off", "--bluetoooth"]).into_iter()).unwrap_err();
        assert!(err.contains("--bluetoooth"), "unexpected: {err}");
        assert!(err.contains("unknown flag"));
    }

    #[test]
    fn unknown_short_flag_is_a_clear_error() {
        let err = parse_opts(args(&["-x", "scan"]).into_iter()).unwrap_err();
        assert!(err.contains("-x"), "unexpected: {err}");
    }

    #[test]
    fn missing_flag_value_is_an_error() {
        for flag in ["--adapter", "--device", "--channel"] {
            let err = parse_opts(args(&["scan", flag]).into_iter()).unwrap_err();
            assert!(err.contains(flag), "unexpected: {err}");
            assert!(err.contains("requires a value"), "unexpected: {err}");
        }
    }

    #[test]
    fn flag_followed_by_another_flag_is_an_error() {
        let err = parse_opts(args(&["--device", "--bluez", "scan"]).into_iter()).unwrap_err();
        assert!(err.contains("--device"), "unexpected: {err}");
    }

    #[test]
    fn unparseable_channel_is_an_error_not_a_silent_default() {
        let err = parse_opts(args(&["--channel", "seven", "scan"]).into_iter()).unwrap_err();
        assert!(err.contains("--channel"), "unexpected: {err}");
        assert!(err.contains("seven"), "unexpected: {err}");
    }

    #[test]
    fn help_flag_sets_help_from_any_position() {
        let (opts, _) = parse_opts(args(&["scan", "--help"]).into_iter()).expect("parse");
        assert!(opts.help);
        let (opts, _) = parse_opts(args(&["-h"]).into_iter()).expect("parse");
        assert!(opts.help);
    }

    /* #59: the anc command maps to the validated 0x17 scene table */

    #[test]
    fn anc_scene_matches_the_validated_anc_setting_table() {
        // Same payloads as qcy_app::core::SimpleNoise::scene, pinned by
        // core_behavior.rs::noise_mode_maps_to_the_validated_anc_setting_table.
        let cases: &[(&str, [u8; 3])] = &[
            ("off", [0x02, 0x00, 0x00]),
            ("anc", [0x01, 0x01, 0x02]),
            ("indoor", [0x01, 0x01, 0x02]),
            ("commuting", [0x01, 0x02, 0x02]),
            ("noisy", [0x01, 0x03, 0x02]),
            ("wind", [0x01, 0x04, 0x02]),
            ("adaptive", [0x01, 0x05, 0x02]),
            ("transparency", [0x03, 0x02, 0x04]),
        ];
        for (mode, payload) in cases {
            let (label, scene) = anc_scene(mode).unwrap_or_else(|| panic!("{mode} maps"));
            assert_eq!(scene, *payload, "{mode} payload");
            assert!(!label.is_empty());
        }
        // The legacy CLI spelling keeps working and maps to indoor ANC.
        let (label, scene) = anc_scene("on").expect("on maps");
        assert_eq!(label, "anc");
        assert_eq!(scene, [0x01, 0x01, 0x02]);
    }

    #[test]
    fn anc_scene_rejects_unknown_modes() {
        assert_eq!(anc_scene("loud"), None);
        assert_eq!(anc_scene(""), None);
    }

    /* #70: write commands require explicit intent */

    #[test]
    fn bare_anc_is_an_error_asking_for_an_explicit_mode() {
        let err = run_args(args(&["anc"])).unwrap_err();
        let TransportError::InvalidArgument(message) = err else {
            panic!("expected InvalidArgument, got {err:?}")
        };
        assert!(
            message.contains("anc requires an explicit mode"),
            "{message}"
        );
    }

    #[test]
    fn unknown_anc_mode_is_an_error_before_any_transport_use() {
        let err = run_args(args(&["anc", "loud"])).unwrap_err();
        let TransportError::InvalidArgument(message) = err else {
            panic!("expected InvalidArgument, got {err:?}")
        };
        assert!(message.contains("unknown anc mode"), "{message}");
    }

    #[test]
    fn bare_game_mode_is_an_error_asking_for_explicit_intent() {
        let err = run_args(args(&["game-mode"])).unwrap_err();
        let TransportError::InvalidArgument(message) = err else {
            panic!("expected InvalidArgument, got {err:?}")
        };
        assert!(
            message.contains("game-mode requires an explicit value"),
            "{message}"
        );
    }

    #[test]
    fn unknown_game_mode_value_is_an_error() {
        let err = run_args(args(&["game-mode", "maybe"])).unwrap_err();
        let TransportError::InvalidArgument(message) = err else {
            panic!("expected InvalidArgument, got {err:?}")
        };
        assert!(message.contains("unknown game-mode value"), "{message}");
    }

    #[test]
    fn unexpected_extra_arguments_are_rejected() {
        let err = run_args(args(&["scan", "leftover"])).unwrap_err();
        let TransportError::InvalidArgument(message) = err else {
            panic!("expected InvalidArgument, got {err:?}")
        };
        assert!(message.contains("unexpected argument"), "{message}");
        assert!(message.contains("leftover"), "{message}");

        let err = run_args(args(&["anc", "off", "extra"])).unwrap_err();
        assert!(matches!(err, TransportError::InvalidArgument(_)));
    }

    #[test]
    fn unparseable_system_eq_gain_is_an_error() {
        let err = run_args(args(&["system-eq", "on", "0.0", "loud"])).unwrap_err();
        let TransportError::InvalidArgument(message) = err else {
            panic!("expected InvalidArgument, got {err:?}")
        };
        assert!(message.contains("not a number"), "{message}");
    }

    #[test]
    fn unknown_command_is_still_an_error() {
        let err = run_args(args(&["frobnicate"])).unwrap_err();
        assert!(matches!(err, TransportError::InvalidArgument(_)));
    }
}
