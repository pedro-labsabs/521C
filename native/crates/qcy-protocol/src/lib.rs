//! Independent QCY BLE protocol codecs.
//!
//! Documented from public reverse-engineering of QCY earphone GATT traffic.
//! Not affiliated with QCY. No proprietary SDK code is included.

pub mod advertisement;
pub mod packet;

pub const SOF: u8 = 0xFF;
pub const QCY_COMPANY_ID: u16 = 0x521C;

/// Opcodes mirrored from the TypeScript catalog. Writability is governed by the
/// canonical evidence ledger (`src/lib/qcy/protocol/evidence.ts` in the web tree);
/// presence here does not make an opcode writable. Destructive opcodes are never
/// issued by unattended automation at any layer.
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

#[cfg(test)]
mod evidence_consistency {
    use super::Cmd;

    /// The destructive set must stay exactly the documented 0x01/0x02/0x03, matching
    /// the TypeScript evidence ledger and `DESTRUCTIVE_CMDS`. This is the native-side
    /// half of the "destructive never automated" invariant.
    #[test]
    fn destructive_set_is_exactly_documented() {
        let destructive: Vec<u8> = [
            Cmd::ResetDefault,
            Cmd::ClearPairing,
            Cmd::FactoryReset,
            Cmd::MusicControl,
            Cmd::LightFlash,
            Cmd::LowLatency,
            Cmd::NoiseCancelMode,
            Cmd::EqParamsV2,
            Cmd::Battery,
            Cmd::RequestData,
        ]
        .iter()
        .filter(|c| c.is_destructive())
        .map(|c| *c as u8)
        .collect();
        assert_eq!(destructive, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn destructive_opcodes_are_flagged() {
        assert!(Cmd::ResetDefault.is_destructive());
        assert!(Cmd::ClearPairing.is_destructive());
        assert!(Cmd::FactoryReset.is_destructive());
        assert!(!Cmd::LowLatency.is_destructive());
        assert!(!Cmd::RequestData.is_destructive());
    }
}
