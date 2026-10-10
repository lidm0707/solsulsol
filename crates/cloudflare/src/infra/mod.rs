//! Worker infra: D1 repositories + pure-Rust adapters + composition root.

pub mod adapters;
pub mod d1_repos;
pub mod r2;
pub mod solana;

use std::str::FromStr;
use std::sync::Arc;

use crate::infra::adapters::{JwtIssuer, PromptPayAdapter, QrAdapter};
use crate::infra::d1_repos::{
    D1DestinationRepo, D1GoodsRepo, D1InviteRepo, D1InvoiceRepo, D1MemberRepo, D1NonceRepo,
    D1PaymentOrderRepo, D1ShopRepo, D1StorageRepo, D1SubscriptionRepo, D1UserRepo,
};
use crate::infra::solana::{Ed25519Verifier, RpcPaymentVerifier};
use pos_app::config::PaymentConfig;
use pos_app::service::use_cases::AppServices;
use rust_decimal::Decimal;

pub const DB_BINDING: &str = "my_db";
pub const JWT_SECRET_ENV: &str = "POS_JWT_SECRET";
pub const DEFAULT_JWT_SECRET: &str = "dev-only-secret-change-me";

pub const VAR_NETWORK: &str = "SOLANA_NETWORK";
pub const VAR_RPC_URL: &str = "SOLANA_RPC_URL";
pub const VAR_USDC_MINT: &str = "USDC_MINT";
pub const VAR_RECIPIENT: &str = "PAYMENT_RECIPIENT";
pub const VAR_DOMAIN: &str = "POS_DOMAIN";
pub const VAR_NONCE_TTL: &str = "NONCE_TTL_SECS";
pub const VAR_GRACE_DAYS: &str = "GRACE_DAYS";
pub const VAR_RECOVERY_DAYS: &str = "RECOVERY_DAYS";
pub const VAR_PRICE_USDC: &str = "SUBSCRIPTION_PRICE_USDC";

pub const DEFAULT_PRICE_USDC: i64 = 1;
const ERR_MISSING_RECIPIENT: &str = "PAYMENT_RECIPIENT is not configured";

fn env_var(env: &worker::Env, key: &str) -> Result<Option<String>, String> {
    // worker::Env::var errors when the binding is undefined; all callers have
    // defaults or explicit missing-config errors, so undefined just means None
    Ok(env.var(key).ok().map(|v| v.to_string()))
}

fn env_i64(env: &worker::Env, key: &str, default: i64) -> Result<i64, String> {
    match env_var(env, key)? {
        Some(v) => v.parse::<i64>().map_err(|e| format!("{key}: {e}")),
        None => Ok(default),
    }
}

pub fn payment_config(env: &worker::Env) -> Result<PaymentConfig, String> {
    let recipient = env_var(env, VAR_RECIPIENT)?.ok_or(ERR_MISSING_RECIPIENT)?;
    let price = match env_var(env, VAR_PRICE_USDC)? {
        Some(v) => Decimal::from_str(&v).map_err(|e| format!("{VAR_PRICE_USDC}: {e}"))?,
        None => Decimal::from(DEFAULT_PRICE_USDC),
    };
    Ok(PaymentConfig {
        network: env_var(env, VAR_NETWORK)?
            .unwrap_or_else(|| pos_app::config::NETWORK_DEVNET.to_owned()),
        rpc_url: env_var(env, VAR_RPC_URL)?.unwrap_or_else(|| {
            format!("https://api.{}.solana.com", pos_app::config::NETWORK_DEVNET)
        }),
        usdc_mint: env_var(env, VAR_USDC_MINT)?
            .unwrap_or_else(|| pos_app::config::USDC_MINT_DEVNET.to_owned()),
        recipient,
        price_usdc: price,
        domain: env_var(env, VAR_DOMAIN)?.unwrap_or_else(|| "localhost".to_owned()),
        nonce_ttl_secs: env_i64(env, VAR_NONCE_TTL, 300)?,
        grace_days: env_i64(env, VAR_GRACE_DAYS, 30)?,
        recovery_days: env_i64(env, VAR_RECOVERY_DAYS, 7)?,
    })
}

pub fn wire_services(env: &worker::Env) -> Result<AppServices, String> {
    let db: d1_repos::Db = Arc::new(env.d1(DB_BINDING).map_err(|e| e.to_string())?);
    let secret = env
        .var(JWT_SECRET_ENV)
        .map(|v| v.to_string())
        .unwrap_or_else(|_| DEFAULT_JWT_SECRET.to_owned());
    let cfg = payment_config(env)?;
    let rpc_url = cfg.rpc_url.clone();
    let builder = AppServices::builder(cfg)
        .users(Arc::new(D1UserRepo { db: db.clone() }))
        .shops(Arc::new(D1ShopRepo { db: db.clone() }))
        .members(Arc::new(D1MemberRepo { db: db.clone() }))
        .goods(Arc::new(D1GoodsRepo { db: db.clone() }))
        .destinations(Arc::new(D1DestinationRepo { db: db.clone() }))
        .invites(Arc::new(D1InviteRepo { db: db.clone() }))
        .invoices(Arc::new(D1InvoiceRepo { db: db.clone() }))
        .nonces(Arc::new(D1NonceRepo { db: db.clone() }))
        .subscriptions(Arc::new(D1SubscriptionRepo { db: db.clone() }))
        .orders(Arc::new(D1PaymentOrderRepo { db: db.clone() }))
        .storage(Arc::new(D1StorageRepo { db: db.clone() }))
        .tokens(Arc::new(JwtIssuer::new(secret)))
        .wallet_verifier(Arc::new(Ed25519Verifier))
        .payment_verifier(Arc::new(RpcPaymentVerifier { rpc_url }))
        .qr(Arc::new(QrAdapter))
        .promptpay(Arc::new(PromptPayAdapter));
    let builder = match crate::infra::r2::R2BindingStorage::from_env(env)? {
        Some(store) => builder.objects(Arc::new(store)),
        None => builder,
    };
    builder.build()
}
