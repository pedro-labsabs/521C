use crate::{BatteryCell, BatteryState, QCY_COMPANY_ID};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advertisement {
    pub vendor_id: u16,
    pub battery: BatteryState,
    pub control_mac: String,
    pub other_mac: String,
}

fn fmt_mac(bytes: [u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]
    )
}

/// Manufacturer data CompanyID 0x521c.
/// Control MAC display order: [12]:[11]:[13]:[16]:[15]:[14]
pub fn parse_manufacturer_data(company_id: u16, data: &[u8]) -> Option<Advertisement> {
    if company_id != QCY_COMPANY_ID || data.len() < 8 {
        return None;
    }
    let vendor_id = u16::from_be_bytes([data[0], data[1]]);
    let battery = BatteryState {
        left: BatteryCell::decode(data[5]),
        right: BatteryCell::decode(data[6]),
        case: BatteryCell::decode(data[7]),
    };
    let mut control_mac = "00:00:00:00:00:00".to_string();
    let mut other_mac = control_mac.clone();
    if data.len() >= 17 {
        control_mac = fmt_mac([data[12], data[11], data[13], data[16], data[15], data[14]]);
    }
    if data.len() >= 24 {
        other_mac = fmt_mac([data[19], data[18], data[20], data[23], data[22], data[21]]);
    }
    if other_mac == "00:00:00:00:00:00" {
        other_mac = control_mac.clone();
    }
    Some(Advertisement {
        vendor_id,
        battery,
        control_mac,
        other_mac,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrambled_mac() {
        let mut d = [0u8; 24];
        d[0] = 0x12;
        d[1] = 0x34;
        d[5] = 80;
        d[6] = 70 | 0x80;
        d[7] = 90;
        d[12] = 0xAA;
        d[11] = 0xBB;
        d[13] = 0xCC;
        d[16] = 0xDD;
        d[15] = 0xEE;
        d[14] = 0xFF;
        let adv = parse_manufacturer_data(0x521C, &d).unwrap();
        assert_eq!(adv.vendor_id, 0x1234);
        assert_eq!(adv.control_mac, "AA:BB:CC:DD:EE:FF");
        assert_eq!(adv.battery.left.level, 80);
        assert!(adv.battery.right.charging);
    }

    #[test]
    fn rejects_other_company() {
        assert!(parse_manufacturer_data(0x004C, &[0u8; 24]).is_none());
    }
}
