//! Shared protocol conformance tests.
//!
//! These tests consume the same `conformance/protocol_vectors.json` corpus as
//! the TypeScript suite (`src/lib/qcy/protocol/conformance.test.ts`). A
//! cross-language semantic divergence covered by a vector fails at least one
//! repository gate. See `conformance/README.md` for schema and provenance.

use qcy_protocol::advertisement::parse_manufacturer_data;
use qcy_protocol::packet::{decode_packet, encode_blocks, CommandBlock, DecodeError};
use qcy_protocol::BatteryState;
use serde::Deserialize;

const VECTORS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../conformance/protocol_vectors.json"
));

fn from_hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("valid hex"))
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Deserialize)]
struct Corpus {
    decode: Vec<DecodeVector>,
    encode: Vec<EncodeVector>,
    advertisement: Vec<AdvVector>,
    battery: Vec<BatteryVector>,
}

#[derive(Deserialize)]
struct DecodeVector {
    name: String,
    hex: String,
    expect: DecodeExpect,
}

#[derive(Deserialize)]
struct DecodeExpect {
    ok: bool,
    #[serde(default)]
    blocks: Vec<BlockVector>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
struct BlockVector {
    cmd: u8,
    #[serde(default, rename = "paramsHex")]
    params_hex: String,
}

#[derive(Deserialize)]
struct EncodeVector {
    name: String,
    blocks: Vec<BlockVector>,
    #[serde(rename = "expectHex")]
    expect_hex: String,
}

#[derive(Deserialize)]
struct AdvVector {
    name: String,
    #[serde(rename = "companyId")]
    company_id: u16,
    #[serde(rename = "dataHex")]
    data_hex: String,
    expect: Option<AdvExpect>,
}

#[derive(Deserialize)]
struct AdvExpect {
    #[serde(rename = "vendorId")]
    vendor_id: u16,
    battery: BatteryExpect,
    #[serde(rename = "controlMac")]
    control_mac: String,
    #[serde(rename = "otherMac")]
    other_mac: String,
}

#[derive(Deserialize)]
struct BatteryVector {
    name: String,
    hex: String,
    expect: Option<BatteryExpect>,
}

#[derive(Deserialize)]
struct BatteryExpect {
    left: CellExpect,
    right: CellExpect,
    case: CellExpect,
}

#[derive(Deserialize)]
struct CellExpect {
    level: u8,
    charging: bool,
}

fn map_error(kind: &str) -> DecodeError {
    match kind {
        "too-short" => DecodeError::TooShort,
        "bad-sof" => DecodeError::BadSof,
        "length-mismatch" => DecodeError::LengthMismatch,
        "truncated-block" => DecodeError::TruncatedBlock,
        "oversize" => DecodeError::Oversize,
        other => panic!("unknown error kind in corpus: {other}"),
    }
}

fn corpus() -> Corpus {
    serde_json::from_str(VECTORS).expect("corpus parses")
}

fn assert_battery(got: &BatteryState, want: &BatteryExpect, ctx: &str) {
    assert_eq!(got.left.level, want.left.level, "{ctx} left level");
    assert_eq!(got.left.charging, want.left.charging, "{ctx} left charging");
    assert_eq!(got.right.level, want.right.level, "{ctx} right level");
    assert_eq!(
        got.right.charging, want.right.charging,
        "{ctx} right charging"
    );
    assert_eq!(got.case.level, want.case.level, "{ctx} case level");
    assert_eq!(got.case.charging, want.case.charging, "{ctx} case charging");
}

#[test]
fn frame_decode_vectors() {
    for v in corpus().decode {
        let bytes = from_hex(&v.hex);
        let result = decode_packet(&bytes);
        if v.expect.ok {
            let pkt = result.unwrap_or_else(|e| panic!("{}: expected ok, got {e:?}", v.name));
            assert_eq!(
                pkt.blocks.len(),
                v.expect.blocks.len(),
                "{} block count",
                v.name
            );
            for (i, want) in v.expect.blocks.iter().enumerate() {
                assert_eq!(pkt.blocks[i].cmd, want.cmd, "{} block {i} cmd", v.name);
                assert_eq!(
                    to_hex(&pkt.blocks[i].params),
                    want.params_hex,
                    "{} block {i} params",
                    v.name
                );
            }
        } else {
            let err = result
                .err()
                .unwrap_or_else(|| panic!("{}: expected error, got ok", v.name));
            let want = map_error(v.expect.error.as_deref().unwrap());
            assert_eq!(err, want, "{} error kind", v.name);
        }
    }
}

#[test]
fn frame_encode_vectors() {
    for v in corpus().encode {
        let blocks: Vec<CommandBlock> = v
            .blocks
            .iter()
            .map(|b| CommandBlock {
                cmd: b.cmd,
                params: from_hex(&b.params_hex),
            })
            .collect();
        let out = encode_blocks(&blocks).unwrap_or_else(|e| panic!("{}: {e:?}", v.name));
        assert_eq!(to_hex(&out), v.expect_hex, "{}", v.name);
    }
}

#[test]
fn advertisement_vectors() {
    for v in corpus().advertisement {
        let data = from_hex(&v.data_hex);
        let adv = parse_manufacturer_data(v.company_id, &data);
        match v.expect {
            Some(want) => {
                let a = adv.unwrap_or_else(|| panic!("{}: expected Some", v.name));
                assert_eq!(a.vendor_id, want.vendor_id, "{} vendorId", v.name);
                assert_battery(&a.battery, &want.battery, &v.name);
                assert_eq!(a.control_mac, want.control_mac, "{} controlMac", v.name);
                assert_eq!(a.other_mac, want.other_mac, "{} otherMac", v.name);
            }
            None => assert!(adv.is_none(), "{}: expected None", v.name),
        }
    }
}

#[test]
fn battery_vectors() {
    for v in corpus().battery {
        let bytes = from_hex(&v.hex);
        let parsed = BatteryState::decode(&bytes);
        match v.expect {
            Some(want) => {
                let b = parsed.unwrap_or_else(|| panic!("{}: expected Some", v.name));
                assert_battery(&b, &want, &v.name);
            }
            None => assert!(parsed.is_none(), "{}: expected None", v.name),
        }
    }
}

#[test]
fn rejects_oversize_buffer() {
    let mut big = vec![0u8; 600];
    big[0] = 0xFF;
    assert_eq!(decode_packet(&big), Err(DecodeError::Oversize));
}

#[test]
fn rejects_overlong_params() {
    let block = CommandBlock {
        cmd: 0x22,
        params: vec![0u8; 256],
    };
    assert_eq!(encode_blocks(&[block]), Err(DecodeError::Oversize));
}

#[test]
fn rejects_overlong_body() {
    let blocks: Vec<CommandBlock> = (0..100)
        .map(|_| CommandBlock {
            cmd: 0x09,
            params: vec![0x01],
        })
        .collect();
    assert_eq!(encode_blocks(&blocks), Err(DecodeError::Oversize));
}
