//! QR rendering port.

use async_trait::async_trait;

#[async_trait]
pub trait QrRenderer: Send + Sync {
    async fn render_png(&self, payload: &str) -> Result<Vec<u8>, String>;
}
