//! Wallet auth: nonce challenge, SIWS-style JSON message, ed25519 verify.

use crate::config::PaymentConfig;
use crate::domain::entities::User;
use crate::domain::entities::subscription::AuthNonce;
use crate::domain::value_objects::{Role, Wallet, new_id};

use crate::port::encryption::TokenIssuer;
use crate::port::solana::MessageVerifier;
use crate::service::use_cases::AppServices;
use chrono::{DateTime, Duration, SubsecRound, Utc};
use uuid::Uuid;

pub const DEFAULT_NONCE_TTL_SECS: i64 = 300;

#[derive(Debug)]
pub enum AuthError {
    Invalid(String),
    Storage(String),
}

impl From<String> for AuthError {
    fn from(e: String) -> Self {
        AuthError::Storage(e)
    }
}

pub const DEFAULT_USER_ROLE: Role = Role::Employee;

/// Sign-in message: JSON binding wallet, nonce, domain and expiry.
#[must_use]
pub fn sign_message(
    cfg: &PaymentConfig,
    wallet: &str,
    nonce: &str,
    expires_at: DateTime<Utc>,
) -> String {
    serde_json::json!({
        "domain": cfg.domain,
        "wallet": wallet,
        "nonce": nonce,
        "expires_at": expires_at.to_rfc3339(),
        "statement": "Sign in to solana-pos",
    })
    .to_string()
}

/// Issues a single-use nonce for `wallet`; returns (nonce, message, expiry).
pub async fn nonce(
    svc: &AppServices,
    wallet: &str,
) -> Result<(String, String, DateTime<Utc>), AuthError> {
    let wallet = Wallet::parse(wallet.trim()).map_err(|e| AuthError::Invalid(e.to_string()))?;
    let nonce = new_nonce_string();
    let ttl = if svc.payment_config.nonce_ttl_secs > 0 {
        svc.payment_config.nonce_ttl_secs
    } else {
        DEFAULT_NONCE_TTL_SECS
    };
    let expires_at = (Utc::now() + Duration::seconds(ttl)).trunc_subsecs(6);
    svc.nonces
        .insert(&AuthNonce {
            id: new_id(),
            wallet: wallet.as_str().to_owned(),
            nonce: nonce.clone(),
            expires_at,
        })
        .await?;
    let msg = sign_message(&svc.payment_config, wallet.as_str(), &nonce, expires_at);
    tracing::info!(wallet = wallet.as_str(), "nonce issued");
    Ok((nonce, msg, expires_at))
}

/// Verifies signature over the message, consumes the nonce and
/// find-or-creates the wallet user, then issues a session token.
pub async fn verify(
    svc: &AppServices,
    verifier: &dyn MessageVerifier,
    tokens: &dyn TokenIssuer,
    wallet: &str,
    nonce: &str,
    signature: &str,
) -> Result<(Uuid, String), AuthError> {
    let wallet = Wallet::parse(wallet.trim()).map_err(|e| AuthError::Invalid(e.to_string()))?;
    let nonce = nonce.trim();
    if nonce.is_empty() || signature.trim().is_empty() {
        return Err(AuthError::Invalid("missing nonce or signature".into()));
    }
    let stored = svc.nonces.find(wallet.as_str(), nonce).await?;
    let Some(record) = stored else {
        return Err(AuthError::Invalid("invalid or expired nonce".into()));
    };
    let message = sign_message(
        &svc.payment_config,
        wallet.as_str(),
        nonce,
        record.expires_at,
    );
    let ok = verifier
        .verify(&message, wallet.as_str(), signature.trim())
        .await?;
    if !ok {
        tracing::warn!(wallet = wallet.as_str(), "wallet signature invalid");
        return Err(AuthError::Invalid("signature verification failed".into()));
    }
    // Single-use: replays and expired nonces are rejected atomically.
    let consumed = svc
        .nonces
        .consume(wallet.as_str(), nonce, Utc::now())
        .await?;
    if !consumed {
        tracing::warn!(
            wallet = wallet.as_str(),
            "nonce rejected (used/expired/unknown)"
        );
        return Err(AuthError::Invalid("invalid or expired nonce".into()));
    }
    let user = match svc.users.find_by_wallet(wallet.as_str()).await? {
        Some(u) => u,
        None => {
            let u = User {
                id: new_id(),
                email: None,
                wallet: Some(wallet.clone()),
                role: DEFAULT_USER_ROLE,
                created_at: Utc::now(),
            };
            svc.users.insert(&u).await?;
            tracing::info!(user_id = %u.id, "wallet user created");
            u
        }
    };
    let token = tokens.issue(user.id, user.role).await?;
    tracing::info!(user_id = %user.id, "wallet sign-in ok");
    Ok((user.id, token))
}

fn new_nonce_string() -> String {
    new_id().to_string()
}
