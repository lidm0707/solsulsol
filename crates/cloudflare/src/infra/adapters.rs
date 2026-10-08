//! Worker-side adapters implementing pos-port traits with pure-Rust crates.

use async_trait::async_trait;
use pos_app::domain::value_objects::Role;
use pos_app::port::{PasswordHasher, PromptPayEncoder, QrRenderer, TokenIssuer};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const TOKEN_TTL_HOURS: i64 = 24;

// --- password hashing ---

pub struct Argon2Hasher;

impl Argon2Hasher {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Argon2Hasher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PasswordHasher for Argon2Hasher {
    async fn hash(&self, plain: &str) -> Result<String, String> {
        use argon2::password_hash::rand_core::OsRng;
        use argon2::password_hash::{PasswordHasher as _, SaltString};
        use argon2::Argon2;

        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(plain.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| e.to_string())
    }

    async fn verify(&self, plain: &str, hash: &str) -> Result<bool, String> {
        use argon2::password_hash::{PasswordHash as PwHash, PasswordVerifier as _};
        use argon2::Argon2;

        let parsed = PwHash::new(hash).map_err(|e| e.to_string())?;
        Ok(Argon2::default()
            .verify_password(plain.as_bytes(), &parsed)
            .is_ok())
    }
}

// --- JWT session tokens ---

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub role: String,
    pub exp: i64,
}

pub struct JwtIssuer {
    secret: String,
}

impl JwtIssuer {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }
}

#[async_trait]
impl TokenIssuer for JwtIssuer {
    async fn issue(&self, user_id: Uuid, role: Role) -> Result<String, String> {
        let claims = Claims {
            sub: user_id,
            role: format!("{role:?}"),
            exp: (chrono::Utc::now() + chrono::Duration::hours(TOKEN_TTL_HOURS)).timestamp(),
        };
        jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(self.secret.as_bytes()),
        )
        .map_err(|e| e.to_string())
    }

    async fn verify(&self, token: &str) -> Result<Uuid, String> {
        let data = jsonwebtoken::decode::<Claims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(self.secret.as_bytes()),
            &jsonwebtoken::Validation::default(),
        )
        .map_err(|e| e.to_string())?;
        Ok(data.claims.sub)
    }
}

// --- QR + PromptPay ---

pub struct QrAdapter;

#[async_trait]
impl QrRenderer for QrAdapter {
    async fn render_png(&self, payload: &str) -> Result<Vec<u8>, String> {
        pos_qr_code::encode::encode_png(payload).map_err(|e| e.to_string())
    }
}

pub struct PromptPayAdapter;

impl PromptPayEncoder for PromptPayAdapter {
    fn target(&self, raw: &str) -> String {
        pos_promptpay::target::target(raw)
    }

    fn encode_payload(&self, target_id: &str, amount: Decimal) -> String {
        pos_promptpay::payload::build_payload(target_id, amount)
    }
}
