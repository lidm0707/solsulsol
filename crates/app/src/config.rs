//! Runtime configuration for wallet auth + subscription payments.

use rust_decimal::Decimal;

/// Mainnet-beta USDC mint (Circle). Devnet USDC differs; see `.env.example`.
pub const USDC_MINT_MAINNET: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
pub const USDC_MINT_DEVNET: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";
pub const NETWORK_DEVNET: &str = "devnet";
pub const NETWORK_MAINNET: &str = "mainnet-beta";

/// Subscription / payment settings. Sourced from env (backend) or
/// `[vars]` (worker); never hardcoded per deployment.
#[derive(Debug, Clone)]
pub struct PaymentConfig {
    pub network: String,
    pub rpc_url: String,
    pub usdc_mint: String,
    pub recipient: String,
    pub price_usdc: Decimal,
    /// Domain embedded in the sign-in message (replay binding).
    pub domain: String,
    pub nonce_ttl_secs: i64,
    pub grace_days: i64,
    /// Soft-deleted data is purged this many days after grace ends.
    pub recovery_days: i64,
}

impl PaymentConfig {
    #[must_use]
    pub fn devnet(recipient: &str) -> Self {
        Self {
            network: NETWORK_DEVNET.to_owned(),
            rpc_url: format!("https://api.{}.solana.com", NETWORK_DEVNET),
            usdc_mint: USDC_MINT_DEVNET.to_owned(),
            recipient: recipient.to_owned(),
            price_usdc: crate::domain::entities::subscription::SUB_PRICE_USDC,
            domain: "localhost".to_owned(),
            nonce_ttl_secs: 300,
            grace_days: 30,
            recovery_days: 7,
        }
    }
}
