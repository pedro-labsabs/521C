//! 521cctl — CLI with the same command surface as the GUI core.
//!
//! Mock transport is the deliberate default (tests/dev, no hardware needed). Pass
//! `--bluez` to operate a real, explicitly selected device through the system BlueZ
//! stack. Outbound writes pass through the central write-authorization policy either way.

use qcy_protocol::packet::encode_command;
use qcy_protocol::Cmd;
use qcy_transport::policy::{WritePolicy, CHAR_COMMAND_WRITE};
use qcy_transport::{mock::MockTransport, DiscoveredDevice, Transport, TransportError};

const CHAR_BATTERY: &str = "00000008-0000-1000-8000-00805f9b34fb";
const CHAR_VERSION: &str = "00000007-0000-1000-8000-00805f9b34fb";

struct Options {
    bluez: bool,
    adapter: String,
    device: Option<String>,
}

fn parse_opts<I: Iterator<Item = String>>(args: &mut I) -> (Options, Vec<String>) {
    let mut opts = Options {
        bluez: false,
        adapter: "hci0".to_string(),
        device: None,
    };
    let mut rest = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--bluez" => opts.bluez = true,
            "--mock" => opts.bluez = false,
            "--adapter" => opts.adapter = args.next().unwrap_or_else(|| "hci0".into()),
            "--device" => opts.device = args.next(),
            _ => {
                rest.push(a);
                rest.extend(args);
                break;
            }
        }
    }
    (opts, rest)
}

fn build_transport(opts: &Options) -> Result<Box<dyn Transport>, TransportError> {
    if opts.bluez {
        build_bluez(opts)
    } else {
        Ok(Box::new(MockTransport::new(WritePolicy::ht08())))
    }
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

fn run() -> Result<(), TransportError> {
    let mut args = std::env::args().skip(1);
    let (opts, rest) = parse_opts(&mut args);
    let cmd = rest.first().cloned().unwrap_or_else(|| "help".into());
    let backend = if opts.bluez { "bluez" } else { "mock" };

    match cmd.as_str() {
        "help" | "-h" | "--help" => {
            println!("521cctl [--mock|--bluez] [--adapter hci0] [--device <addr>] <command>");
            println!("  scan                     list candidate QCY devices");
            println!("  connect [<addr>]         connect and resolve characteristics");
            println!("  status                   connect + battery/firmware readout");
            println!("  battery                  connect + battery readout");
            println!("  anc <off|on|transparency>");
            println!("  game-mode <on|off>");
            println!("Mock is the deliberate default; --bluez targets a real device.");
            println!("Independent. Not affiliated with QCY.");
        }
        "scan" => {
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
            let mut t = build_transport(&opts)?;
            let list = t.scan()?;
            let dev = pick_device(
                &list,
                rest.get(1).map(|s| s.as_str()).or(opts.device.as_deref()),
            )?;
            t.connect(&dev.address)?;
            println!("connected to {} ({}) via {backend}", dev.name, dev.address);
        }
        "status" | "battery" => {
            let mut t = build_transport(&opts)?;
            let list = t.scan()?;
            let dev = pick_device(&list, opts.device.as_deref())?;
            if !dev.model_known {
                println!("note: model not proven; treating as read-only");
            }
            t.connect(&dev.address)?;
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
            let mode = rest.get(1).cloned().unwrap_or_else(|| "on".into());
            let v: u8 = match mode.as_str() {
                "off" => 0x00,
                "transparency" => 0x03,
                _ => 0x01,
            };
            let mut t = build_transport(&opts)?;
            let list = t.scan()?;
            let dev = pick_device(&list, opts.device.as_deref())?;
            t.connect(&dev.address)?;
            let frame = encode_command(Cmd::NoiseCancelMode as u8, &[v])
                .map_err(|e| TransportError::InvalidArgument(format!("{e:?}")))?;
            t.write(&frame)?;
            println!("anc {mode} -> 0x{v:02x}");
        }
        "game-mode" => {
            let on = rest.get(1).map(|s| s.as_str()) != Some("off");
            let mut t = build_transport(&opts)?;
            let list = t.scan()?;
            let dev = pick_device(&list, opts.device.as_deref())?;
            t.connect(&dev.address)?;
            let frame = encode_command(Cmd::LowLatency as u8, &[if on { 0x01 } else { 0x02 }])
                .map_err(|e| TransportError::InvalidArgument(format!("{e:?}")))?;
            t.write(&frame)?;
            println!("game-mode {}", if on { "on" } else { "off" });
        }
        other => {
            eprintln!("unknown command: {other}");
            std::process::exit(1);
        }
    }
    // Keep the command characteristic referenced so the allowlist is exercised in builds.
    let _ = CHAR_COMMAND_WRITE;
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
