//! Composition root helpers: wiring adapters into ports.
//! Only this module may depend on every layer.

use std::sync::Arc;

use crate::app::config::PaymentConfig;
use crate::app::service::use_cases::{AppServices, AppServicesBuilder};
use crate::infra::database_adapter::repos;
use crate::infra::encryption_adapter::jwt::JwtIssuer;
use crate::infra::qr_adapter::adapter::QrAdapter;
use crate::infra::solana_adapter::rpc::RpcPaymentVerifier;
use crate::infra::solana_adapter::signature::Ed25519Verifier;
use crate::infra::storage_adapter::local::FsObjectStorage;

pub const JWT_SECRET_ENV: &str = "POS_JWT_SECRET";
pub const DATABASE_URL_ENV: &str = "POS_DATABASE_URL";
pub const DEFAULT_JWT_SECRET: &str = "dev-only-secret-change-me";
pub const SOLANA_NETWORK_ENV: &str = "SOLANA_NETWORK";
pub const SOLANA_RPC_URL_ENV: &str = "SOLANA_RPC_URL";
pub const USDC_MINT_ENV: &str = "USDC_MINT";
pub const PAYMENT_RECIPIENT_ENV: &str = "PAYMENT_RECIPIENT";
pub const POS_DOMAIN_ENV: &str = "POS_DOMAIN";
pub const NONCE_TTL_SECS_ENV: &str = "NONCE_TTL_SECS";
pub const GRACE_DAYS_ENV: &str = "GRACE_DAYS";
pub const RECOVERY_DAYS_ENV: &str = "RECOVERY_DAYS";
pub const DEFAULT_DOMAIN: &str = "localhost";
pub const DEFAULT_RPC_URL: &str = "https://api.devnet.solana.com";
pub const DEFAULT_NONCE_TTL_SECS: i64 = 300;
pub const DEFAULT_GRACE_DAYS: i64 = 7;
pub const DEFAULT_RECOVERY_DAYS: i64 = 7;
pub const STORAGE_DIR_ENV: &str = "POS_STORAGE_DIR";
pub const MAX_UPLOAD_BYTES_ENV: &str = "POS_MAX_UPLOAD_BYTES";
/// Devnet USDC test mint; goodsion must override `USDC_MINT`.
pub const DEFAULT_USDC_MINT: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

fn payment_config_from_env() -> Result<PaymentConfig, String> {
    let recipient = std::env::var(PAYMENT_RECIPIENT_ENV)
        .map_err(|_| format!("{PAYMENT_RECIPIENT_ENV} must be set (shop payment wallet)"))?;
    let mut cfg = PaymentConfig::devnet(&recipient);
    cfg.domain = std::env::var(POS_DOMAIN_ENV).unwrap_or_else(|_| DEFAULT_DOMAIN.to_owned());
    if let Ok(url) = std::env::var(SOLANA_RPC_URL_ENV) {
        cfg.rpc_url = url;
    }
    if let Ok(mint) = std::env::var(USDC_MINT_ENV) {
        cfg.usdc_mint = mint;
    }
    if let Ok(net) = std::env::var(SOLANA_NETWORK_ENV) {
        cfg.network = net;
    }
    if let Ok(v) = std::env::var(NONCE_TTL_SECS_ENV) {
        cfg.nonce_ttl_secs = v.parse().map_err(|_| format!("bad {NONCE_TTL_SECS_ENV}"))?;
    }
    if let Ok(v) = std::env::var(GRACE_DAYS_ENV) {
        cfg.grace_days = v.parse().map_err(|_| format!("bad {GRACE_DAYS_ENV}"))?;
    }
    if let Ok(v) = std::env::var(RECOVERY_DAYS_ENV) {
        cfg.recovery_days = v.parse().map_err(|_| format!("bad {RECOVERY_DAYS_ENV}"))?;
    }
    Ok(cfg)
}

fn storage_dir_from_env() -> Result<Option<String>, String> {
    match std::env::var(STORAGE_DIR_ENV) {
        Ok(dir) if dir.trim().is_empty() => Err(format!("{STORAGE_DIR_ENV} must not be empty")),
        Ok(dir) => Ok(Some(dir)),
        Err(_) => Ok(None),
    }
}

fn max_upload_bytes_from_env() -> Result<Option<i64>, String> {
    match std::env::var(MAX_UPLOAD_BYTES_ENV) {
        Ok(v) => v
            .parse::<i64>()
            .map(Some)
            .map_err(|_| format!("{MAX_UPLOAD_BYTES_ENV} must be a positive integer")),
        Err(_) => Ok(None),
    }
}

pub async fn wire_services() -> Result<AppServices, String> {
    let cfg = payment_config_from_env()?;
    let secret = std::env::var(JWT_SECRET_ENV).unwrap_or_else(|_| {
        tracing::warn!("{} not set, using dev secret", JWT_SECRET_ENV);
        DEFAULT_JWT_SECRET.to_owned()
    });
    let url = std::env::var(DATABASE_URL_ENV)
        .map_err(|_| format!("{} must be set (postgres)", DATABASE_URL_ENV))?;
    let builder = AppServices::builder(cfg)
        .tokens(Arc::new(JwtIssuer::new(secret)))
        .wallet_verifier(Arc::new(Ed25519Verifier))
        .payment_verifier(Arc::new(RpcPaymentVerifier::new(
            &std::env::var(SOLANA_RPC_URL_ENV).unwrap_or_else(|_| DEFAULT_RPC_URL.to_owned()),
        )))
        .qr(Arc::new(QrAdapter))
        .promptpay(Arc::new(
            crate::infra::promptpay_adapter::adapter::PromptPayAdapter,
        ));
    let builder = match storage_dir_from_env()? {
        Some(dir) => {
            tracing::info!(root = %dir, "filesystem object storage enabled");
            builder.objects(Arc::new(FsObjectStorage::new(
                &dir,
                max_upload_bytes_from_env()?,
            )))
        }
        None => {
            tracing::info!("{} not set, object endpoints disabled", STORAGE_DIR_ENV);
            builder
        }
    };
    wire_postgres(builder, &url).await
}

async fn wire_postgres(builder: AppServicesBuilder, url: &str) -> Result<AppServices, String> {
    let pool = repos::connect(url).await.map_err(|e| e.to_string())?;
    run_migrations(&pool).await?;
    builder
        .users(Arc::new(repos::PgUserRepo { pool: pool.clone() }))
        .shops(Arc::new(repos::PgShopRepo { pool: pool.clone() }))
        .members(Arc::new(repos::PgMemberRepo { pool: pool.clone() }))
        .goods(Arc::new(repos::PgGoodsRepo { pool: pool.clone() }))
        .destinations(Arc::new(repos::PgDestinationRepo { pool: pool.clone() }))
        .invites(Arc::new(repos::PgInviteRepo { pool: pool.clone() }))
        .billings(Arc::new(repos::PgBillingRepo { pool: pool.clone() }))
        .nonces(Arc::new(repos::PgNonceRepo { pool: pool.clone() }))
        .subscriptions(Arc::new(repos::PgSubscriptionRepo { pool: pool.clone() }))
        .orders(Arc::new(repos::PgPaymentOrderRepo { pool: pool.clone() }))
        .storage(Arc::new(repos::PgStorageRepo { pool: pool.clone() }))
        .tx(Arc::new(repos::PgTxManager { pool }))
        .build()
}

async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), String> {
    sqlx::migrate!("../../docker/migrations")
        .run(pool)
        .await
        .map_err(|e| e.to_string())
}
