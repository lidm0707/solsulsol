//! Argon2id password hashing adapter.

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{
    PasswordHash as PwHash, PasswordHasher as _, PasswordVerifier as _, SaltString,
};
use async_trait::async_trait;
use crate::port::PasswordHasher;

#[derive(Default)]
pub struct Argon2Hasher;

const MIN_SENSIBLE_LEN: usize = 4;

impl Argon2Hasher {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl PasswordHasher for Argon2Hasher {
    async fn hash(&self, plain: &str) -> Result<String, String> {
        if plain.len() < MIN_SENSIBLE_LEN {
            return Err("password too short".into());
        }
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(plain.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| e.to_string())
    }

    async fn verify(&self, plain: &str, hash: &str) -> Result<bool, String> {
        let parsed = PwHash::new(hash).map_err(|e| e.to_string())?;
        Ok(Argon2::default()
            .verify_password(plain.as_bytes(), &parsed)
            .is_ok())
    }
}
