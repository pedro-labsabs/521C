//! 521cctl — same command surface as the GUI core, mock transport by default.
//! Real BlueZ/GATT I/O is intentionally out of this sandbox binary.

use qcy_protocol::packet::{decode_packet, encode_command};
use qcy_protocol::{BatteryCell, BatteryState, Cmd};

struct MockHt08 {
    battery: BatteryState,
    noise: u8,
    game: bool,
    name: &'static str,
}

impl MockHt08 {
    fn new() -> Self {
        Self {
            battery: BatteryState {
                left: BatteryCell {
                    level: 82,
                    charging: false,
                },
                right: BatteryCell {
                    level: 80,
                    charging: false,
                },
                case: BatteryCell {
                    level: 94,
                    charging: false,
                },
            },
            noise: 0x01,
            game: false,
            name: "QCY MeloBuds Pro",
        }
    }

    fn apply(&mut self, bytes: &[u8]) {
        if let Ok(pkt) = decode_packet(bytes) {
            for b in pkt.blocks {
                match b.cmd {
                    x if x == Cmd::NoiseCancelMode as u8 && !b.params.is_empty() => {
                        self.noise = b.params[0];
                    }
                    x if x == Cmd::LowLatency as u8 && !b.params.is_empty() => {
                        self.game = b.params[0] == 0x01;
                    }
                    _ => {}
                }
            }
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "help".into());
    let mut dev = MockHt08::new();
    match cmd.as_str() {
        "help" | "-h" | "--help" => {
            println!(
                "521cctl status|battery|anc <mode>|game-mode <on|off>\nIndependent. Not affiliated with QCY."
            );
        }
        "status" => {
            println!("{}  HT08  mock", dev.name);
            println!(
                "noise=0x{n:02x} game={g}  L={l}% R={r}% case={c}%",
                n = dev.noise,
                g = if dev.game { "on" } else { "off" },
                l = dev.battery.left.level,
                r = dev.battery.right.level,
                c = dev.battery.case.level
            );
        }
        "battery" => {
            println!(
                "L {}%  R {}%  case {}%",
                dev.battery.left.level, dev.battery.right.level, dev.battery.case.level
            );
        }
        "anc" => {
            let mode = args.next().unwrap_or_else(|| "on".into());
            let v = match mode.as_str() {
                "off" => 0x00,
                "adaptive" | "on" | "anc" => 0x01,
                "transparency" => 0x03,
                _ => 0x01,
            };
            let pkt = encode_command(Cmd::NoiseCancelMode as u8, &[v]).unwrap();
            dev.apply(&pkt);
            println!("anc {mode} -> 0x{v:02x}  frame={pkt:02x?}");
        }
        "game-mode" => {
            let on = args.next().as_deref() != Some("off");
            let pkt =
                encode_command(Cmd::LowLatency as u8, &[if on { 0x01 } else { 0x02 }]).unwrap();
            dev.apply(&pkt);
            println!("game-mode {}", if on { "on" } else { "off" });
        }
        other => {
            eprintln!("unknown command: {other}");
            std::process::exit(1);
        }
    }
}
