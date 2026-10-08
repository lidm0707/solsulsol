//! Barcode → SVG. Pure module-pattern rendering, no external encoder.

use crate::error::BarcodeError;
use crate::format::{self, BarcodeFormat};

pub const MODULE_WIDTH: u32 = 2;
pub const BAR_HEIGHT: u32 = 60;
pub const QUIET_ZONE_MODULES: usize = 9;
pub const SVG_PAD: u32 = 4;

/// Render a validated barcode as a black-on-white SVG string.
pub fn encode_svg(raw: &str) -> Result<String, BarcodeError> {
    let code = format::normalize(raw);
    let fmt = format::validate(raw)?;
    let modules = modules(&code, fmt)?;
    let width = (modules.len() * MODULE_WIDTH as usize
        + QUIET_ZONE_MODULES * 2 * MODULE_WIDTH as usize) as u32;
    let mut svg = String::with_capacity(modules.len() * 48 + 128);
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{}\" viewBox=\"0 0 {width} {}\">",
        BAR_HEIGHT + SVG_PAD * 2,
        BAR_HEIGHT + SVG_PAD * 2
    ));
    svg.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#fff\"/>");
    let mut x = u32::try_from(QUIET_ZONE_MODULES).unwrap_or(0) * MODULE_WIDTH;
    for m in modules {
        if m {
            svg.push_str(&format!(
                "<rect x=\"{x}\" y=\"{SVG_PAD}\" width=\"{MODULE_WIDTH}\" height=\"{BAR_HEIGHT}\" fill=\"#000\"/>"
            ));
        }
        x += MODULE_WIDTH;
    }
    svg.push_str("</svg>");
    Ok(svg)
}

/// Bit pattern per module: true = bar, false = space (quiet zones excluded).
fn modules(code: &str, fmt: BarcodeFormat) -> Result<Vec<bool>, BarcodeError> {
    match fmt {
        BarcodeFormat::Ean13 => Ok(ean13_modules(code)),
        BarcodeFormat::Ean8 => Ok(ean8_modules(code)),
        BarcodeFormat::UpcA => Ok(ean13_modules(&format!("0{code}"))),
        BarcodeFormat::Code39 => {
            code39_modules(code).ok_or(BarcodeError::BadCode39Char { ch: '*' })
        }
    }
}

const L_GUGE: [u8; 7] = [0, 0, 0, 1, 1, 0, 1];
const L_GGLL: [u8; 7] = [0, 0, 1, 1, 0, 0, 1];
const L_GGLG: [u8; 7] = [0, 1, 0, 0, 1, 1, 1];
const L_GLLG: [u8; 7] = [0, 1, 1, 1, 0, 1, 0];
const L_GLGG: [u8; 7] = [1, 0, 0, 0, 1, 1, 0];
const L_LGGL: [u8; 7] = [1, 1, 0, 0, 0, 1, 0];
const L_LGGG: [u8; 7] = [1, 0, 1, 1, 1, 0, 0];
const L_LLGL: [u8; 7] = [1, 1, 1, 0, 1, 0, 1];
const L_LLGG: [u8; 7] = [1, 1, 0, 1, 1, 1, 0];
const L_LLGG2: [u8; 7] = [1, 0, 0, 1, 0, 0, 1];

const L_CODE: [&[u8; 7]; 10] = [
    &L_GUGE, &L_GGLL, &L_GGLG, &L_GLLG, &L_GLGG, &L_LGGL, &L_LGGG, &L_LLGL, &L_LLGG, &L_LLGG2,
];

const G_CODE: [&[u8; 7]; 10] = [
    &L_GGLL, &L_GGLG, &L_GLLG, &L_GLGG, &L_LGGL, &L_LGGG, &L_LLGL, &L_LLGG, &L_LLGG2, &L_GUGE,
];

const R_CODE: [&[u8; 7]; 10] = [
    &L_LLGG2, &L_LLGG, &L_LLGL, &L_LGGG, &L_LGGL, &L_GLGG, &L_GLLG, &L_GGLG, &L_GGLL, &L_GUGE,
];

// EAN-13 parity: first digit selects L/G pattern for digits 2..7.
const PARITY: [&[u8; 6]; 10] = [
    &[0, 0, 0, 0, 0, 0],
    &[0, 0, 1, 0, 1, 1],
    &[0, 0, 1, 1, 0, 1],
    &[0, 0, 1, 1, 1, 0],
    &[0, 1, 0, 0, 1, 1],
    &[0, 1, 1, 0, 0, 1],
    &[0, 1, 1, 1, 0, 0],
    &[0, 1, 0, 1, 0, 1],
    &[0, 1, 0, 1, 1, 0],
    &[0, 0, 0, 1, 0, 1],
];

fn digit(c: char) -> usize {
    usize::from(c as u8 - b'0')
}

fn push_bars(out: &mut Vec<bool>, code: &[(usize, char)]) {
    for table in code {
        let (set, ch) = *table;
        let d = digit(ch);
        let row = match set {
            0 => &L_CODE[d],
            1 => &G_CODE[d],
            _ => &R_CODE[d],
        };
        out.extend(row.iter().map(|b| *b == 1));
    }
}

fn guard(out: &mut Vec<bool>, modules: usize) {
    for i in 0..modules {
        out.push(i % 2 == 0);
    }
}

/// EAN-13: 3 + 42 + 5 + 42 + 3 = 95 modules.
fn ean13_modules(code: &str) -> Vec<bool> {
    let digits: Vec<char> = code.chars().collect();
    let mut out = Vec::with_capacity(95);
    guard(&mut out, 3);
    let parity = PARITY[digit(digits[0])];
    let left: Vec<(usize, char)> = (1..7)
        .map(|i| (usize::from(parity[i - 1] == 1), digits[i]))
        .collect();
    push_bars(&mut out, &left);
    guard(&mut out, 5);
    let right: Vec<(usize, char)> = (7..13).map(|i| (2, digits[i])).collect();
    push_bars(&mut out, &right);
    guard(&mut out, 3);
    out
}

/// EAN-8: 3 + 28 + 5 + 28 + 3 = 67 modules.
fn ean8_modules(code: &str) -> Vec<bool> {
    let digits: Vec<char> = code.chars().collect();
    let mut out = Vec::with_capacity(67);
    guard(&mut out, 3);
    let left: Vec<(usize, char)> = (0..4).map(|i| (0, digits[i])).collect();
    push_bars(&mut out, &left);
    guard(&mut out, 5);
    let right: Vec<(usize, char)> = (4..8).map(|i| (2, digits[i])).collect();
    push_bars(&mut out, &right);
    guard(&mut out, 3);
    out
}

const CODE39_TABLE: [(&str, &str); 44] = [
    ("0", "101001101101"),
    ("1", "110100101011"),
    ("2", "101100101011"),
    ("3", "110110010101"),
    ("4", "101001101011"),
    ("5", "110100110101"),
    ("6", "101100110101"),
    ("7", "101001011011"),
    ("8", "110100101101"),
    ("9", "101100101101"),
    ("A", "110101001011"),
    ("B", "101101001011"),
    ("C", "110110100101"),
    ("D", "101011001011"),
    ("E", "110101100101"),
    ("F", "101101100101"),
    ("G", "101010011011"),
    ("H", "110101001101"),
    ("I", "101101001101"),
    ("J", "101011001101"),
    ("K", "110101010011"),
    ("L", "101101010011"),
    ("M", "110110101001"),
    ("N", "101011010011"),
    ("O", "110101101001"),
    ("P", "101101101001"),
    ("Q", "101010110011"),
    ("R", "110101011001"),
    ("S", "101101011001"),
    ("T", "101011011001"),
    ("U", "110010101011"),
    ("V", "100110101011"),
    ("W", "110011010101"),
    ("X", "100101101011"),
    ("Y", "110010110101"),
    ("Z", "100110110101"),
    ("-", "100101011011"),
    (".", "110010101101"),
    (" ", "100110101101"),
    ("$", "100100100101"),
    ("/", "100100101001"),
    ("+", "100101001001"),
    ("%", "101001001001"),
    ("*", "100101101101"),
];

fn code39_modules(code: &str) -> Option<Vec<bool>> {
    let mut out = Vec::new();
    let pattern = |ch: char| -> Option<&'static str> {
        CODE39_TABLE
            .iter()
            .find(|(k, _)| *k == ch.to_string())
            .map(|(_, v)| *v)
    };
    out.extend(bits(pattern('*')?));
    for ch in code.chars() {
        out.extend(bits(pattern(ch)?));
    }
    out.extend(bits(pattern('*')?));
    Some(out)
}

fn bits(pattern: &str) -> impl Iterator<Item = bool> + '_ {
    pattern.chars().map(|c| c == '1')
}
