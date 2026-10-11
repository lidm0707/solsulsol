//! Barcode string validation, normalization and checksums.
//! Supported symbologies: EAN-13, EAN-8, UPC-A, Code39.

use crate::error::{
    BarcodeError, CODE39_MAX_CHARS, CODE39_MIN_CHARS, EAN8_DIGITS, EAN13_DIGITS, UPC_DIGITS,
};
use crate::format::BarcodeFormat::{Code39, Ean8, Ean13, UpcA};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarcodeFormat {
    Ean13,
    Ean8,
    UpcA,
    Code39,
}

impl BarcodeFormat {
    #[must_use]
    pub const fn digits(self) -> Option<usize> {
        match self {
            Self::Ean13 => Some(EAN13_DIGITS),
            Self::Ean8 => Some(EAN8_DIGITS),
            Self::UpcA => Some(UPC_DIGITS),
            Self::Code39 => None,
        }
    }
}

/// UPC-A is EAN-13 with a leading 0; normalize it to EAN-13.
#[must_use]
pub fn normalize(raw: &str) -> String {
    raw.trim().to_ascii_uppercase()
}

/// Detect the symbology of raw input and validate its checksum.
/// Every goods barcode must pass this before it is stored or scanned.
pub fn validate(raw: &str) -> Result<BarcodeFormat, BarcodeError> {
    let code = normalize(raw);
    if code.is_empty() {
        return Err(BarcodeError::Empty);
    }
    match code.len() {
        EAN13_DIGITS => {
            expect_digits(&code)?;
            checksum(&code, EAN13_DIGITS)
                .then_some(Ean13)
                .ok_or(BarcodeError::BadChecksum { format: Ean13 })
        }
        EAN8_DIGITS => {
            expect_digits(&code)?;
            checksum(&code, EAN8_DIGITS)
                .then_some(Ean8)
                .ok_or(BarcodeError::BadChecksum { format: Ean8 })
        }
        UPC_DIGITS => {
            expect_digits(&code)?;
            // UPC-A is EAN-13 with a leading 0; one checksum covers both forms.
            if checksum(&format!("0{code}"), EAN13_DIGITS) {
                Ok(UpcA)
            } else {
                Err(BarcodeError::BadChecksum { format: UpcA })
            }
        }
        _ if code.chars().all(is_code39_char) => validate_code39(&code),
        _ => Err(BarcodeError::NotDigits),
    }
}

/// Validate raw input as a specific symbology.
pub fn validate_as(format: BarcodeFormat, raw: &str) -> Result<BarcodeFormat, BarcodeError> {
    let code = normalize(raw);
    if code.is_empty() {
        return Err(BarcodeError::Empty);
    }
    let Some(digits) = format.digits() else {
        return validate_code39(&code);
    };
    if code.len() != digits {
        return Err(BarcodeError::BadLength {
            expected: digits,
            got: code.len(),
        });
    }
    expect_digits(&code)?;
    if code.len() == EAN8_DIGITS {
        checksum(&code, EAN8_DIGITS)
            .then_some(Ean8)
            .ok_or(BarcodeError::BadChecksum { format: Ean8 })
    } else {
        // UPC-A is EAN-13 with a leading 0; one checksum covers both forms.
        let extended = if format == UpcA {
            format!("0{code}")
        } else {
            code
        };
        checksum(&extended, EAN13_DIGITS)
            .then_some(format)
            .ok_or(BarcodeError::BadChecksum { format })
    }
}

fn validate_code39(code: &str) -> Result<BarcodeFormat, BarcodeError> {
    let len = code.chars().count();
    if !(CODE39_MIN_CHARS..=CODE39_MAX_CHARS).contains(&len) {
        return Err(BarcodeError::Code39Length {
            min: CODE39_MIN_CHARS,
            max: CODE39_MAX_CHARS,
        });
    }
    code.chars().try_for_each(|c| {
        is_code39_char(c)
            .then_some(())
            .ok_or(BarcodeError::BadCode39Char { ch: c })
    })?;
    Ok(Code39)
}

fn expect_digits(code: &str) -> Result<(), BarcodeError> {
    code.chars().try_for_each(|c| {
        c.is_ascii_digit()
            .then_some(())
            .ok_or(BarcodeError::NotDigits)
    })
}

/// EAN checksum: digits alternate weight 1/3 from the right (last digit excluded).
#[must_use]
pub fn checksum(digits: &str, len: usize) -> bool {
    let Some(without_check) = digits.get(..len - 1) else {
        return false;
    };
    let sum: u32 = without_check
        .chars()
        .rev()
        .enumerate()
        .map(|(i, c)| u32::from(c as u8 - b'0') * if i % 2 == 0 { 3 } else { 1 })
        .sum();
    let check = (10 - (sum % 10)) % 10;
    check == u32::from(digits.as_bytes()[len - 1] - b'0')
}

#[must_use]
pub const fn is_code39_char(c: char) -> bool {
    matches!(c,
        '0'..='9' | 'A'..='Z' | '-' | '.' | ' ' | '$' | '/' | '+' | '%')
}
