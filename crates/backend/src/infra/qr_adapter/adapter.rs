//! QR adapter: implements pos-app QrRenderer with pos-qr-code.

use async_trait::async_trait;
use crate::port::QrRenderer;

pub struct QrAdapter;

#[async_trait]
impl QrRenderer for QrAdapter {
    async fn render_png(&self, payload: &str) -> Result<Vec<u8>, String> {
        tracing::info!(bytes = payload.len(), "rendering qr");
        pos_qr_code::encode::encode_png(payload).map_err(|e| e.to_string())
    }
}
