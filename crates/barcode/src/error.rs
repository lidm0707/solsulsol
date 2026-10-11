use thiserror::Error;

pub const EAN13_DIGITS: usize = 13;
pub const EAN8_DIGITS: usize = 8;
pub const UPC_DIGITS: usize = 12;
pub const CODE39_MIN_CHARS: usize = 1;
pub const CODE39_MAX_CHARS: usize = 43;

#[derive(Debug, Error)]
pub enum BarcodeError {
    #[error("barcode must not be empty")]
    Empty,
    #[error("expected {expected} digits, got {got}")]
    BadLength { expected: usize, got: usize },
    #[error("barcode must contain digits only")]
    NotDigits,
    #[error("checksum mismatch for {format:?}")]
    BadChecksum {
        format: crate::format::BarcodeFormat,
    },
    #[error("invalid code39 character: {ch}")]
    BadCode39Char { ch: char },
    #[error("code39 needs {min}..={max} chars")]
    Code39Length { min: usize, max: usize },
}
