//! Solana ports: wallet signature verification + on-chain payment verification.

use rust_decimal::Decimal;
use thiserror::Error;

#[async_trait::async_trait]
pub trait MessageVerifier: Send + Sync {
    /// Ed25519-verify `signature` (base58) over `message` by `wallet`.
    async fn verify(&self, message: &str, wallet: &str, signature: &str) -> Result<bool, String>;
}

#[derive(Debug, Error)]
pub enum PaymentVerifyError {
    #[error("transaction not found (not yet finalized or bad signature)")]
    NotFound,
    #[error("transaction failed on chain")]
    Failed,
    #[error("payment did not go to the expected recipient")]
    WrongRecipient,
    #[error("payment was not in the expected token")]
    WrongMint,
    #[error("payment amount does not match the order")]
    WrongAmount,
    #[error("transaction is not finalized yet")]
    Unfinalized,
    #[error("rpc error: {0}")]
    Rpc(String),
}

#[derive(Debug, Clone)]
pub struct VerifiedPayment {
    pub amount: Decimal,
    pub slot: i64,
}

#[async_trait::async_trait]
pub trait PaymentVerifier: Send + Sync {
    /// Verifies an on-chain USDC transfer: successful status, expected
    /// recipient, expected mint, at least `amount`, finalized commitment.
    async fn verify_usdc_transfer(
        &self,
        signature: &str,
        recipient: &str,
        mint: &str,
        amount: Decimal,
    ) -> Result<VerifiedPayment, PaymentVerifyError>;
}
