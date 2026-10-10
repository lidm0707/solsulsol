//! Ed25519 wallet-signature verification (base58 signatures, SIWS message).

use ed25519_dalek::Signature;
use ed25519_dalek::VerifyingKey;
use pos_app::port::solana::MessageVerifier;

pub struct Ed25519Verifier;

const SIG_B58_LEN_MAX: usize = 100;

#[async_trait::async_trait]
impl MessageVerifier for Ed25519Verifier {
    async fn verify(&self, message: &str, wallet: &str, signature: &str) -> Result<bool, String> {
        let pubkey_vec = bs58::decode(wallet).into_vec().map_err(|e| e.to_string())?;
        let pubkey: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = pubkey_vec
            .as_slice()
            .try_into()
            .map_err(|_| "wallet is not an ed25519 pubkey".to_owned())?;
        let sig_bytes = bs58::decode(signature)
            .into_vec()
            .map_err(|e| e.to_string())?;
        if sig_bytes.len() > SIG_B58_LEN_MAX {
            return Ok(false);
        }
        let sig = Signature::from_slice(&sig_bytes).map_err(|e| e.to_string())?;
        let key = VerifyingKey::from_bytes(&pubkey).map_err(|e| e.to_string())?;
        Ok(key.verify_strict(message.as_bytes(), &sig).is_ok())
    }
}
