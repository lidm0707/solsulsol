//! Payload → QR code in PNG or SVG form.

use crate::error::{MAX_QR_BYTES, PNG_MIN_SIZE, QrError, SVG_PIXEL_SIZE};
use qrcode::QrCode;
use qrcode::render::svg;

pub fn encode_png(payload: &str) -> Result<Vec<u8>, QrError> {
    let code = validated(payload)?;
    let img = code
        .render::<image::Luma<u8>>()
        .min_dimensions(PNG_MIN_SIZE, PNG_MIN_SIZE)
        .build();
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma8(img)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|_| QrError::TooLong)?;
    Ok(bytes.into_inner())
}

pub fn encode_svg(payload: &str) -> Result<String, QrError> {
    let code = validated(payload)?;
    Ok(code
        .render::<svg::Color>()
        .module_dimensions(SVG_PIXEL_SIZE, SVG_PIXEL_SIZE)
        .build())
}

fn validated(payload: &str) -> Result<QrCode, QrError> {
    if payload.is_empty() {
        return Err(QrError::Empty);
    }
    if payload.len() > MAX_QR_BYTES {
        return Err(QrError::TooLong);
    }
    QrCode::new(payload.as_bytes()).map_err(|_| QrError::TooLong)
}
