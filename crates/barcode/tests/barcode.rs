use pos_barcode::format::{self, BarcodeFormat};
use pos_barcode::render::encode_svg;

const EAN13_VALID: &str = "4006381333931";
const EAN8_VALID: &str = "73513537";
const UPC_VALID: &str = "036000291452";
const EAN13_BAD_CHECKSUM: &str = "4006381333932";

#[test]
fn detects_ean13() {
    assert_eq!(
        format::validate(EAN13_VALID).ok(),
        Some(BarcodeFormat::Ean13)
    );
}

#[test]
fn detects_ean8() {
    assert_eq!(format::validate(EAN8_VALID).ok(), Some(BarcodeFormat::Ean8));
}

#[test]
fn detects_upca_as_normalized_ean13() {
    assert_eq!(format::validate(UPC_VALID).ok(), Some(BarcodeFormat::UpcA));
}

#[test]
fn rejects_bad_checksum() {
    assert!(format::validate(EAN13_BAD_CHECKSUM).is_err());
    assert!(format::validate("73513538").is_err());
}

#[test]
fn accepts_code39_and_rejects_bad_chars() {
    assert_eq!(format::validate("POS-1").ok(), Some(BarcodeFormat::Code39));
    assert!(format::validate("pös").is_err());
}

#[test]
fn trims_and_uppercases() {
    assert_eq!(format::normalize(" 4006381333931 "), EAN13_VALID);
    assert_eq!(format::normalize("pos-1"), "POS-1");
}

#[test]
fn renders_ean13_svg() {
    let svg = encode_svg(EAN13_VALID).expect("renders");
    assert!(svg.starts_with("<svg"));
    // 95 modules total, roughly half black.
    let bars = svg.matches("fill=\"#000\"").count();
    assert!((40..=55).contains(&bars), "unexpected bar count {bars}");
}

#[test]
fn renders_code39_with_guards() {
    let svg = encode_svg("POS-1").expect("renders");
    assert!(svg.contains("viewBox"));
}

#[test]
fn rejects_render_of_invalid() {
    assert!(encode_svg(EAN13_BAD_CHECKSUM).is_err());
    assert!(encode_svg("").is_err());
}
