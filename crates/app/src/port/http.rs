//! HTTP ports: outbound calls from backend to external services.

use async_trait::async_trait;

#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn send_json(&self, url: &str, body: &str) -> Result<String, String>;
}
