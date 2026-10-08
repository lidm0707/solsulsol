//! Encryption ports: password hashing + token issuing.

use async_trait::async_trait;
use crate::domain::value_objects::Role;
use uuid::Uuid;

#[async_trait]
pub trait PasswordHasher: Send + Sync {
    async fn hash(&self, plain: &str) -> Result<String, String>;
    async fn verify(&self, plain: &str, hash: &str) -> Result<bool, String>;
}

#[async_trait]
pub trait TokenIssuer: Send + Sync {
    async fn issue(&self, user_id: Uuid, role: Role) -> Result<String, String>;
    async fn verify(&self, token: &str) -> Result<Uuid, String>;
}
