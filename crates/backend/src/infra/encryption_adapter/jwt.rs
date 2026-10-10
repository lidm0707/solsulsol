//! JWT session tokens.

use crate::domain::value_objects::Role;
use crate::port::encryption::TokenIssuer;
use async_trait::async_trait;
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const TOKEN_TTL_HOURS: i64 = 24;

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
            exp: (Utc::now() + Duration::hours(TOKEN_TTL_HOURS)).timestamp(),
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
        .map_err(|e| e.to_string())
    }

    async fn verify(&self, token: &str) -> Result<Uuid, String> {
        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|e| e.to_string())?;
        Ok(data.claims.sub)
    }
}
