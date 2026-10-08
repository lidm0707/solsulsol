use pos_promptpay::payload::{build_from_raw, crc16};
use rust_decimal::Decimal;

#[test]
fn phone_normalizes_to_intl() {
    let p = build_from_raw("081-234-5678", Decimal::new(0, 0));
    assert!(p.contains("01130066812345678"));
}

#[test]
fn national_id_target_kept() {
    let p = build_from_raw("1234567890123", Decimal::new(0, 0));
    assert!(p.contains("01131234567890123"));
}

#[test]
fn payload_crc_self_consistent() {
    let p = build_from_raw("0812345678", Decimal::new(15000, 2));
    assert!(p.starts_with("000201"));
    assert!(p.contains("5303764"));
    assert!(p.contains("5406150.00"));
    let body = &p[..p.len() - 4];
    assert_eq!(&p[p.len() - 4..], &crc16(body));
}

#[test]
fn ewallet_target_kept() {
    let p = build_from_raw("1234567890123456", Decimal::new(0, 0));
    assert!(p.contains("01161234567890123456"));
}

#[test]
fn phone_with_dashes_and_full_payload_structure() {
    let p = build_from_raw("081-234-5678", Decimal::new(15000, 2));
    let expected_body = concat!(
        "000201",
        "010212",
        "29370016A00000067701011101130066812345678",
        "52040000",
        "5303764",
        "5406150.00",
        "5802TH",
        "6304",
    );
    let body = &p[..p.len() - 4];
    assert_eq!(body, expected_body);
    assert_eq!(&p[p.len() - 4..], &crc16(body));
}
