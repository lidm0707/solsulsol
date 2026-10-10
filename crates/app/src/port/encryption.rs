//! Encryption ports: token issuing (wallet auth needs no password hashing).

use crate::domain::value_objects::Role;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait TokenIssuer: Send + Sync {
    async fn issue(&self, user_id: Uuid, role: Role) -> Result<String, String>;
    async fn verify(&self, token: &str) -> Result<Uuid, String>;
}
