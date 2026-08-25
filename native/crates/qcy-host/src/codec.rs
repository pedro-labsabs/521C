//! Codec / sample-rate / profile state from the host audio graph (issue #13).
//!
//! The earbuds do not expose the host's active Bluetooth codec or sample rate over the
//! vendor protocol; that state lives in the Linux audio stack (BlueZ A2DP endpoint +
//! PipeWire). Because it is not reliably portable to read, the contract here is explicit:
//! report a field only when it can be sourced, otherwise report it as unknown. Unknown is
//! an acceptable, honest state — inventing a codec is not.

use crate::HostError;

/// What is known about the active host audio path. Every field is optional; `None`
/// means "unknown", never "absent" or a guessed default.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CodecInfo {
    /// Active codec name (e.g. "sbc", "aac", "aptx", "ldac"), if known.
    pub codec: Option<String>,
    /// Active sample rate in Hz, if known.
    pub sample_rate_hz: Option<u32>,
    /// Audio profile (e.g. "a2dp-sink", "headset-head-unit"), if known.
    pub profile: Option<String>,
}

impl CodecInfo {
    /// Fully unknown — the safe default when no reliable source is available.
    pub fn unknown() -> Self {
        Self::default()
    }

    /// True when nothing is known; the UI should render "--" rather than a value.
    pub fn is_unknown(&self) -> bool {
        self.codec.is_none() && self.sample_rate_hz.is_none() && self.profile.is_none()
    }
}

/// A source of codec state. Implementations must return `Ok(unknown)` rather than an
/// error when the audio stack is present but the field simply is not exposed, and must
/// never fabricate a value.
pub trait CodecSource {
    fn read(&self) -> Result<CodecInfo, HostError>;
}

/// A codec source that always reports unknown. This is the graceful default on hosts
/// where no reliable codec source is wired up, and the baseline for tests.
#[derive(Default)]
pub struct UnknownCodecSource;

impl CodecSource for UnknownCodecSource {
    fn read(&self) -> Result<CodecInfo, HostError> {
        Ok(CodecInfo::unknown())
    }
}

/// A fixed codec source, used by tests and by callers that have already sourced the
/// values elsewhere (e.g. a future PipeWire/BlueZ reader).
pub struct StaticCodecSource(pub CodecInfo);

impl CodecSource for StaticCodecSource {
    fn read(&self) -> Result<CodecInfo, HostError> {
        Ok(self.0.clone())
    }
}

/* ------------------------------------------------------------------ */
/* BlueZ A2DP transport source (feature = "dbus")                      */
/* ------------------------------------------------------------------ */
/*
 * Codec facts live in the host Bluetooth stack, not in the QCY vendor protocol.
 * BlueZ exposes each A2DP stream as an `org.bluez.MediaTransport1` object with
 * read-only `UUID` (profile), `Codec` (A2DP assigned number), `Configuration`
 * (codec-specific blob) and `State` properties (see BlueZ doc/org.bluez.MediaTransport.rst).
 * Reading them is passive observation over the system bus — no device write, no
 * daemon reconfiguration. Constants below are taken from BlueZ
 * `profiles/audio/a2dp-codecs.h`; nothing is invented.
 */

/// A2DP codec assigned numbers (BlueZ `a2dp-codecs.h`).
pub const A2DP_CODEC_SBC: u8 = 0x00;
pub const A2DP_CODEC_MPEG12: u8 = 0x01;
pub const A2DP_CODEC_MPEG24_AAC: u8 = 0x02;
pub const A2DP_CODEC_ATRAC: u8 = 0x04;
pub const A2DP_CODEC_VENDOR: u8 = 0xFF;

/// Vendor IDs (BlueZ `a2dp-codecs.h`).
pub const VENDOR_APTX: u32 = 0x0000004f;
pub const VENDOR_FASTSTREAM: u32 = 0x0000000a;
pub const VENDOR_APTX_HD: u32 = 0x000000d7;
pub const VENDOR_LDAC: u32 = 0x0000012d;
pub const VENDOR_OPUS_G: u32 = 0x000000e0;

/// Codec IDs within vendor space (BlueZ `a2dp-codecs.h`).
pub const CODEC_ID_APTX: u16 = 0x0001;
pub const CODEC_ID_FASTSTREAM: u16 = 0x0001;
pub const CODEC_ID_APTX_LL: u16 = 0x0002;
pub const CODEC_ID_APTX_HD: u16 = 0x0024;
pub const CODEC_ID_LDAC: u16 = 0x00aa;
pub const CODEC_ID_OPUS_G: u16 = 0x0001;

/// Raw facts for one A2DP transport as reported by BlueZ. All fields are exactly
/// what the stack exposes; interpretation happens in the pure functions below.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct A2dpTransport {
    /// Profile UUID of the transport (e.g. `0000110b-...` A2DP Sink).
    pub uuid: String,
    /// A2DP codec assigned number.
    pub codec: u8,
    /// Codec-specific configuration blob.
    pub configuration: Vec<u8>,
    /// Transport state: `idle`, `pending`, `broadcasting` or `active`.
    pub state: String,
}

/// A source of A2DP transport facts. Implemented against the live system bus by
/// [`ZbusCodecBus`] (feature `dbus`) and by fakes in unit tests.
pub trait CodecBus: Send {
    fn a2dp_transports(&self) -> Result<Vec<A2dpTransport>, HostError>;
}

/// Pick the transport that best represents what the user is hearing right now:
/// an actively streaming transport wins, then a pending one, then any transport.
pub fn select_transport(transports: &[A2dpTransport]) -> Option<&A2dpTransport> {
    transports
        .iter()
        .find(|t| t.state == "active")
        .or_else(|| transports.iter().find(|t| t.state == "pending"))
        .or_else(|| transports.first())
}

/// Parse the 4-byte little-endian vendor ID and 2-byte little-endian codec ID from
/// the head of a vendor codec configuration blob (BlueZ `a2dp_vendor_codec_t`).
pub fn vendor_ids(configuration: &[u8]) -> Option<(u32, u16)> {
    if configuration.len() < 6 {
        return None;
    }
    let vendor = u32::from_le_bytes([
        configuration[0],
        configuration[1],
        configuration[2],
        configuration[3],
    ]);
    let codec = u16::from_le_bytes([configuration[4], configuration[5]]);
    Some((vendor, codec))
}

/// Human-readable codec name for a transport. `None` means unknown — never guessed.
pub fn codec_name(transport: &A2dpTransport) -> Option<String> {
    match transport.codec {
        A2DP_CODEC_SBC => Some("sbc".into()),
        A2DP_CODEC_MPEG12 => Some("mpeg-1,2-audio".into()),
        A2DP_CODEC_MPEG24_AAC => Some("aac".into()),
        A2DP_CODEC_ATRAC => Some("atrac".into()),
        A2DP_CODEC_VENDOR => {
            let (vendor, codec) = vendor_ids(&transport.configuration)?;
            match (vendor, codec) {
                (VENDOR_APTX, CODEC_ID_APTX) => Some("aptx".into()),
                (VENDOR_FASTSTREAM, CODEC_ID_FASTSTREAM) => Some("faststream".into()),
                (VENDOR_FASTSTREAM, CODEC_ID_APTX_LL) => Some("aptx-ll".into()),
                (VENDOR_APTX_HD, CODEC_ID_APTX_HD) => Some("aptx-hd".into()),
                (VENDOR_LDAC, CODEC_ID_LDAC) => Some("ldac".into()),
                (VENDOR_OPUS_G, CODEC_ID_OPUS_G) => Some("opus-g".into()),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Sample rate in Hz parsed from the codec configuration blob, when the layout is
/// known. Bit values follow BlueZ `a2dp-codecs.h` (SBC/MPEG frequency bitmaps,
/// AAC 12-bit frequency, LDAC frequency byte).
pub fn sample_rate_hz(transport: &A2dpTransport) -> Option<u32> {
    let cfg = &transport.configuration;
    match transport.codec {
        A2DP_CODEC_SBC => {
            // a2dp_sbc_t octet 0: channel_mode:4 | frequency:4 (single selected bit).
            let freq = *cfg.first()? & 0x0F;
            match freq {
                0x01 => Some(48000),
                0x02 => Some(44100),
                0x04 => Some(32000),
                0x08 => Some(16000),
                _ => None,
            }
        }
        A2DP_CODEC_MPEG12 => {
            // a2dp_mpeg_t octet 1: frequency:6 | mpf:1 | rfa:1.
            let freq = *cfg.get(1)? & 0x3F;
            match freq {
                0x01 => Some(48000),
                0x02 => Some(44100),
                0x04 => Some(32000),
                0x08 => Some(24000),
                0x10 => Some(22050),
                0x20 => Some(16000),
                _ => None,
            }
        }
        A2DP_CODEC_MPEG24_AAC => {
            // a2dp_aac_t: 12-bit frequency across octets 1-2 (frequency1 << 4 | frequency2).
            let freq = (u16::from(*cfg.get(1)?) << 4) | u16::from(*cfg.get(2)? >> 4);
            match freq {
                0x0008 => Some(48000),
                0x0010 => Some(44100),
                0x0020 => Some(32000),
                0x0040 => Some(24000),
                0x0080 => Some(22050),
                0x0100 => Some(16000),
                0x0200 => Some(12000),
                0x0400 => Some(11025),
                0x0800 => Some(8000),
                0x0004 => Some(64000),
                0x0002 => Some(88200),
                0x0001 => Some(96000),
                _ => None,
            }
        }
        A2DP_CODEC_VENDOR => {
            let (vendor, codec) = vendor_ids(cfg)?;
            match (vendor, codec) {
                // a2dp_ldac_t: vendor info (6 bytes) then a frequency byte.
                (VENDOR_LDAC, CODEC_ID_LDAC) => match *cfg.get(6)? {
                    0x10 => Some(48000),
                    0x20 => Some(44100),
                    0x08 => Some(88200),
                    0x04 => Some(96000),
                    0x02 => Some(176400),
                    0x01 => Some(192000),
                    _ => None,
                },
                // Other vendor codecs have no reliably portable rate field here.
                _ => None,
            }
        }
        _ => None,
    }
}

/// Audio profile name from the transport's profile UUID. Only well-known A2DP/HFP/HSP
/// UUIDs are named; anything else stays unknown.
pub fn profile_from_uuid(uuid: &str) -> Option<String> {
    let lower = uuid.to_lowercase();
    if lower.starts_with("0000110a-") {
        Some("a2dp-source".into())
    } else if lower.starts_with("0000110b-") {
        Some("a2dp-sink".into())
    } else if lower.starts_with("00001108-") {
        Some("headset-hs".into())
    } else if lower.starts_with("0000111e-") {
        Some("handsfree-hf".into())
    } else {
        None
    }
}

/// Codec source backed by BlueZ `MediaTransport1` objects. Reports only what the
/// stack exposes; every field that cannot be sourced stays `None` (unknown).
pub struct BluezCodecSource {
    bus: Box<dyn CodecBus>,
}

impl BluezCodecSource {
    pub fn new(bus: Box<dyn CodecBus>) -> Self {
        Self { bus }
    }
}

impl CodecSource for BluezCodecSource {
    fn read(&self) -> Result<CodecInfo, HostError> {
        let transports = self.bus.a2dp_transports()?;
        let Some(transport) = select_transport(&transports) else {
            // No A2DP transport present (nothing streaming/paired for A2DP): honest
            // unknown, not an error.
            return Ok(CodecInfo::unknown());
        };
        Ok(CodecInfo {
            codec: codec_name(transport),
            sample_rate_hz: sample_rate_hz(transport),
            profile: profile_from_uuid(&transport.uuid),
        })
    }
}

#[cfg(feature = "dbus")]
pub struct ZbusCodecBus {
    conn: zbus::blocking::Connection,
}

#[cfg(feature = "dbus")]
impl ZbusCodecBus {
    /// Connect to the system bus. Fails with [`HostError::ServiceUnavailable`] when
    /// BlueZ/the system bus is not reachable.
    pub fn system() -> Result<Self, HostError> {
        let conn = zbus::blocking::Connection::system()
            .map_err(|e| HostError::ServiceUnavailable(e.to_string()))?;
        Ok(Self { conn })
    }
}

#[cfg(feature = "dbus")]
impl CodecBus for ZbusCodecBus {
    fn a2dp_transports(&self) -> Result<Vec<A2dpTransport>, HostError> {
        use std::collections::HashMap;
        use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

        let proxy = zbus::blocking::Proxy::new(
            &self.conn,
            "org.bluez",
            "/",
            "org.freedesktop.DBus.ObjectManager",
        )
        .map_err(|e| HostError::ServiceUnavailable(e.to_string()))?;
        let reply = proxy
            .call_method("GetManagedObjects", &())
            .map_err(|e| HostError::Backend(e.to_string()))?;
        let body = reply.body();
        let objects: HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>> = body
            .deserialize()
            .map_err(|e| HostError::Backend(e.to_string()))?;

        let mut out = Vec::new();
        for (_path, ifaces) in objects {
            let Some(props) = ifaces.get("org.bluez.MediaTransport1") else {
                continue;
            };
            let string = |key: &str| -> Option<String> {
                match props.get(key).map(|v| &**v) {
                    Some(Value::Str(s)) => Some(s.to_string()),
                    _ => None,
                }
            };
            let codec = match props.get("Codec").map(|v| &**v) {
                Some(Value::U8(b)) => *b,
                _ => continue,
            };
            let configuration = match props.get("Configuration").map(|v| &**v) {
                Some(Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|item| match item {
                        Value::U8(b) => Some(*b),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            out.push(A2dpTransport {
                uuid: string("UUID").unwrap_or_default(),
                codec,
                configuration,
                state: string("State").unwrap_or_default(),
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_is_the_honest_default() {
        let info = UnknownCodecSource.read().unwrap();
        assert!(info.is_unknown());
        assert_eq!(info.codec, None);
        assert_eq!(info.sample_rate_hz, None);
        assert_eq!(info.profile, None);
    }

    #[test]
    fn static_source_reports_what_it_was_given() {
        let src = StaticCodecSource(CodecInfo {
            codec: Some("ldac".into()),
            sample_rate_hz: Some(96000),
            profile: Some("a2dp-sink".into()),
        });
        let info = src.read().unwrap();
        assert!(!info.is_unknown());
        assert_eq!(info.codec.as_deref(), Some("ldac"));
        assert_eq!(info.sample_rate_hz, Some(96000));
    }

    #[test]
    fn partial_info_is_not_unknown() {
        let info = CodecInfo {
            codec: Some("aac".into()),
            ..Default::default()
        };
        assert!(!info.is_unknown());
    }

    fn transport(codec: u8, configuration: Vec<u8>, state: &str) -> A2dpTransport {
        A2dpTransport {
            uuid: "0000110b-0000-1000-8000-00805f9b34fb".into(),
            codec,
            configuration,
            state: state.into(),
        }
    }

    #[test]
    fn sbc_name_and_rate_from_configuration() {
        // SBC, 48 kHz selected (frequency nibble 0x01), channel mode stereo.
        let t = transport(A2DP_CODEC_SBC, vec![0x21, 0x15, 0x02, 0x35], "active");
        assert_eq!(codec_name(&t).as_deref(), Some("sbc"));
        assert_eq!(sample_rate_hz(&t), Some(48000));
        let t44 = transport(A2DP_CODEC_SBC, vec![0x22, 0x15, 0x02, 0x35], "active");
        assert_eq!(sample_rate_hz(&t44), Some(44100));
    }

    #[test]
    fn aac_name_and_rate_from_configuration() {
        // AAC: object type octet, then 12-bit frequency 0x008 (48000) across octets 1-2.
        let t = transport(
            A2DP_CODEC_MPEG24_AAC,
            vec![0x40, 0x00, 0x80, 0x01, 0x00, 0x00],
            "active",
        );
        assert_eq!(codec_name(&t).as_deref(), Some("aac"));
        assert_eq!(sample_rate_hz(&t), Some(48000));
        let t44 = transport(
            A2DP_CODEC_MPEG24_AAC,
            vec![0x40, 0x01, 0x00, 0x01, 0x00, 0x00],
            "active",
        );
        assert_eq!(sample_rate_hz(&t44), Some(44100));
    }

    #[test]
    fn mpeg_name_and_rate_from_configuration() {
        // MPEG-1,2 Audio: octet 1 frequency bitmap 0x02 = 44100.
        let t = transport(A2DP_CODEC_MPEG12, vec![0x01, 0x02, 0x00, 0x00], "active");
        assert_eq!(codec_name(&t).as_deref(), Some("mpeg-1,2-audio"));
        assert_eq!(sample_rate_hz(&t), Some(44100));
    }

    #[test]
    fn vendor_codecs_are_named_from_le_vendor_and_codec_ids() {
        // LDAC: vendor 0x0000012d LE, codec 0x00aa LE, then frequency byte 0x10 = 48000.
        let ldac = transport(
            A2DP_CODEC_VENDOR,
            vec![0x2d, 0x01, 0x00, 0x00, 0xaa, 0x00, 0x10, 0x01],
            "active",
        );
        assert_eq!(codec_name(&ldac).as_deref(), Some("ldac"));
        assert_eq!(sample_rate_hz(&ldac), Some(48000));
        // aptX: vendor 0x0000004f LE, codec 0x0001 LE.
        let aptx = transport(
            A2DP_CODEC_VENDOR,
            vec![0x4f, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00],
            "active",
        );
        assert_eq!(codec_name(&aptx).as_deref(), Some("aptx"));
        assert_eq!(sample_rate_hz(&aptx), None); // no portable rate field
                                                 // aptX HD: vendor 0x000000d7 LE, codec 0x0024 LE.
        let aptx_hd = transport(
            A2DP_CODEC_VENDOR,
            vec![0xd7, 0x00, 0x00, 0x00, 0x24, 0x00],
            "active",
        );
        assert_eq!(codec_name(&aptx_hd).as_deref(), Some("aptx-hd"));
    }

    #[test]
    fn unknown_vendor_codec_stays_unknown() {
        let t = transport(
            A2DP_CODEC_VENDOR,
            vec![0x99, 0x99, 0x00, 0x00, 0x77, 0x00],
            "active",
        );
        assert_eq!(codec_name(&t), None);
        assert_eq!(sample_rate_hz(&t), None);
    }

    #[test]
    fn truncated_configuration_never_panics_and_stays_unknown() {
        let t = transport(A2DP_CODEC_SBC, vec![], "active");
        assert_eq!(sample_rate_hz(&t), None);
        let t = transport(A2DP_CODEC_VENDOR, vec![0x2d, 0x01], "active");
        assert_eq!(codec_name(&t), None);
    }

    #[test]
    fn profile_uuids_map_to_known_names() {
        assert_eq!(
            profile_from_uuid("0000110b-0000-1000-8000-00805f9b34fb").as_deref(),
            Some("a2dp-sink")
        );
        assert_eq!(
            profile_from_uuid("0000110a-0000-1000-8000-00805f9b34fb").as_deref(),
            Some("a2dp-source")
        );
        assert_eq!(
            profile_from_uuid("0000abcd-0000-1000-8000-00805f9b34fb"),
            None
        );
    }

    #[test]
    fn active_transport_is_preferred() {
        let idle = transport(A2DP_CODEC_SBC, vec![0x21], "idle");
        let active = transport(A2DP_CODEC_MPEG24_AAC, vec![0x40, 0x00, 0x80], "active");
        let pending = transport(A2DP_CODEC_SBC, vec![0x22], "pending");
        let all = vec![idle.clone(), pending.clone(), active.clone()];
        assert_eq!(select_transport(&all), Some(&active));
        assert_eq!(
            select_transport(&[idle.clone(), pending.clone()]),
            Some(&pending)
        );
        assert_eq!(select_transport(std::slice::from_ref(&idle)), Some(&idle));
        assert_eq!(select_transport(&[]), None);
    }

    #[test]
    fn bluez_source_reports_unknown_without_transports() {
        struct EmptyBus;
        impl CodecBus for EmptyBus {
            fn a2dp_transports(&self) -> Result<Vec<A2dpTransport>, HostError> {
                Ok(vec![])
            }
        }
        let src = BluezCodecSource::new(Box::new(EmptyBus));
        let info = src.read().unwrap();
        assert!(info.is_unknown());
    }

    #[test]
    fn bluez_source_maps_active_transport_fields() {
        struct OneBus;
        impl CodecBus for OneBus {
            fn a2dp_transports(&self) -> Result<Vec<A2dpTransport>, HostError> {
                Ok(vec![A2dpTransport {
                    uuid: "0000110b-0000-1000-8000-00805f9b34fb".into(),
                    codec: A2DP_CODEC_SBC,
                    configuration: vec![0x21, 0x15, 0x02, 0x35],
                    state: "active".into(),
                }])
            }
        }
        let src = BluezCodecSource::new(Box::new(OneBus));
        let info = src.read().unwrap();
        assert_eq!(info.codec.as_deref(), Some("sbc"));
        assert_eq!(info.sample_rate_hz, Some(48000));
        assert_eq!(info.profile.as_deref(), Some("a2dp-sink"));
    }

    #[test]
    fn bluez_source_survives_unavailable_bus() {
        struct DeadBus;
        impl CodecBus for DeadBus {
            fn a2dp_transports(&self) -> Result<Vec<A2dpTransport>, HostError> {
                Err(HostError::ServiceUnavailable("no system bus".into()))
            }
        }
        let src = BluezCodecSource::new(Box::new(DeadBus));
        assert!(matches!(src.read(), Err(HostError::ServiceUnavailable(_))));
    }
}
