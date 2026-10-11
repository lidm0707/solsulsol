//! PromptPay EMVCo QR payload per Thai BOT standard.

use crate::target;
use rust_decimal::Decimal;

pub const AID_PROMPTPAY: &str = "A000000677010111";
pub const COUNTRY_CODE_TH: &str = "TH";
pub const CURRENCY_THB: &str = "764";
pub const MERCHANT_CATEGORY: &str = "0000";
pub const PAYLOAD_FORMAT: &str = "01";
pub const POINT_OF_INITIATION_DYNAMIC: &str = "12";
pub const CRC_PREFIX: &str = "6304";
pub const CRC_POLY: u16 = 0x1021;
pub const CRC_INIT: u16 = 0xFFFF;
pub const AMOUNT_DP: u32 = 2;

fn tlv(tag: &str, value: &str) -> String {
    format!("{tag}{:02}{value}", value.len())
}

pub fn crc16(data: &str) -> String {
    let mut crc = CRC_INIT;
    for byte in data.bytes() {
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ CRC_POLY
            } else {
                crc << 1
            };
        }
    }
    format!("{crc:04X}")
}

pub fn build_payload(target_id: &str, amount: Decimal) -> String {
    let amount = format!("{:.*}", AMOUNT_DP as usize, amount.round_dp(AMOUNT_DP));
    let mut payload = String::with_capacity(128);
    payload.push_str(&tlv("00", PAYLOAD_FORMAT));
    payload.push_str(&tlv("01", POINT_OF_INITIATION_DYNAMIC));
    payload.push_str(&tlv(
        "29",
        &format!("{}{}", tlv("00", AID_PROMPTPAY), tlv("01", target_id)),
    ));
    payload.push_str(&tlv("52", MERCHANT_CATEGORY));
    payload.push_str(&tlv("53", CURRENCY_THB));
    payload.push_str(&tlv("54", &amount));
    payload.push_str(&tlv("58", COUNTRY_CODE_TH));
    payload.push_str(CRC_PREFIX);
    payload.push_str(&crc16(&payload));
    payload
}

pub fn build_from_raw(raw_target: &str, amount: Decimal) -> String {
    build_payload(&target::target(raw_target), amount)
}
