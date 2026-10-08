use thiserror::Error;

#[derive(Debug, Error)]
pub enum QrError {
    #[error("payload too long, max {MAX_QR_BYTES} bytes")]
    TooLong,
    #[error("payload must not be empty")]
    Empty,
}

pub const MAX_QR_BYTES: usize = 2953;
pub const PNG_MIN_SIZE: u32 = 4;
pub const SVG_PIXEL_SIZE: u32 = 8;
