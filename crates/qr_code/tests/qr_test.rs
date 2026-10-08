//! PNG/SVG round-trip and validation tests.

use pos_qr_code::encode::{encode_png, encode_svg};

const PAYLOAD: &str = "pay:sol:4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR:01890a5d";

#[test]
fn png_output_is_valid_png() {
    let bytes = encode_png(PAYLOAD).expect("payload encodes");
    assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']), "PNG magic");
    assert!(bytes.len() > 100);
}

#[test]
fn svg_output_is_valid_svg() {
    let svg = encode_svg(PAYLOAD).expect("payload encodes");
    assert!(svg.starts_with("<?xml"));
    assert!(svg.contains("<svg"));
    assert!(svg.contains("path"));
}

#[test]
fn empty_payload_rejected() {
    assert!(encode_png("").is_err());
    assert!(encode_svg("").is_err());
}

#[test]
fn oversize_payload_rejected() {
    let big = "x".repeat(pos_qr_code::error::MAX_QR_BYTES + 1);
    assert!(encode_png(&big).is_err());
    assert!(encode_svg(&big).is_err());
}

#[test]
fn large_payload_encodes() {
    let large = "x".repeat(2000);
    assert!(encode_svg(&large).is_ok());
}
