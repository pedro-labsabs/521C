//! Independent QCY BLE protocol codecs.
//!
//! Documented from public reverse-engineering of QCY earphone GATT traffic.
//! Not affiliated with QCY. No proprietary SDK code is included.

pub mod advertisement;
pub mod packet;

pub const SOF: u8 = 0xFF;
pub const QCY_COMPANY_ID: u16 = 0x521C;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Cmd {
    ResetDefault = 0x01,
    ClearPairing = 0x02,
    FactoryReset = 0x03,
    MusicControl = 0x04,
    LightFlash = 0x05,
    InEarDetection = 0x06,
    Volume = 0x08,
    LowLatency = 0x09,
    NoiseCancelMode = 0x0C,
    SleepMode = 0x10,
    AncSetting = 0x17,
    EqParamsV2 = 0x22,
    Ldac = 0x23,
    KeyFunction = 0x2B,
    WearingDetection = 0x2C,
    SpatialAudio = 0x2D,
    Battery = 0x2F,
    Version = 0x30,
    EnvAdaptation = 0x32,
    TonePlay = 0x3D,
    RequestData = 0xFE,
}

impl Cmd {
    pub fn is_destructive(self) -> bool {
        matches!(
            self,
            Cmd::ResetDefault | Cmd::ClearPairing | Cmd::FactoryReset
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Supported,
    Unsupported,
    Experimental,
    Unknown,
    RequiresProtocolResearch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryCell {
    pub level: u8,
    pub charging: bool,
}

impl BatteryCell {
    pub fn decode(b: u8) -> Self {
        Self {
            level: (b & 0x7F).min(100),
            charging: b & 0x80 != 0,
        }
    }

    pub fn encode(&self) -> u8 {
        let level = self.level.min(127);
        if self.charging {
            level | 0x80
        } else {
            level
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryState {
    pub left: BatteryCell,
    pub right: BatteryCell,
    pub case: BatteryCell,
}

impl BatteryState {
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 3 {
            return None;
        }
        Some(Self {
            left: BatteryCell::decode(bytes[0]),
            right: BatteryCell::decode(bytes[1]),
            case: BatteryCell::decode(bytes[2]),
        })
    }
}
