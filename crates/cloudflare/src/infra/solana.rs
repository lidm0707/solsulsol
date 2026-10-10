//! Solana adapters: ed25519 wallet-signature check + JSON-RPC USDC transfer check.

use std::str::FromStr;

use async_trait::async_trait;
use pos_app::port::solana::{
    MessageVerifier, PaymentVerifier, PaymentVerifyError, VerifiedPayment,
};
use rust_decimal::Decimal;
use serde::Deserialize;

const JSONRPC_METHOD_GET_TX: &str = "getTransaction";
const JSONRPC_ENCODING: &str = "jsonParsed";
const JSONRPC_COMMITMENT: &str = "finalized";
const JSONRPC_MAX_TX_VERSION: i64 = 0;
const RPC_ID: i32 = 1;
const SIGNATURE_LEN: usize = 64;

pub struct Ed25519Verifier;

#[async_trait]
impl MessageVerifier for Ed25519Verifier {
    async fn verify(&self, message: &str, wallet: &str, signature: &str) -> Result<bool, String> {
        use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};

        let sig_bytes = bs58::decode(signature)
            .into_vec()
            .map_err(|e| e.to_string())?;
        let pk_bytes = bs58::decode(wallet).into_vec().map_err(|e| e.to_string())?;
        let pk: [u8; 32] = pk_bytes
            .try_into()
            .map_err(|_| "bad wallet key".to_string())?;
        let vk = VerifyingKey::from_bytes(&pk).map_err(|e| e.to_string())?;
        if sig_bytes.len() != SIGNATURE_LEN {
            return Ok(false);
        }
        let sig = Signature::from_slice(&sig_bytes).map_err(|e| e.to_string())?;
        Ok(vk.verify(message.as_bytes(), &sig).is_ok())
    }
}

pub struct RpcPaymentVerifier {
    pub rpc_url: String,
}

#[derive(serde::Serialize)]
struct RpcRequest {
    jsonrpc: &'static str,
    id: i32,
    method: &'static str,
    params: serde_json::Value,
}

#[derive(Deserialize)]
struct RpcResponse {
    result: Option<TxInfo>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct TxInfo {
    slot: i64,
    meta: Option<TxMeta>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TxMeta {
    err: Option<serde_json::Value>,
    #[serde(default)]
    pre_token_balances: Vec<TokenBalance>,
    #[serde(default)]
    post_token_balances: Vec<TokenBalance>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokenBalance {
    account_index: u32,
    mint: String,
    owner: Option<String>,
    ui_token_amount: UiTokenAmount,
}

#[derive(Deserialize)]
struct UiTokenAmount {
    amount: Option<String>,
    ui_amount_string: Option<String>,
}

impl TokenBalance {
    fn amount(&self) -> Decimal {
        let raw = self
            .ui_token_amount
            .ui_amount_string
            .as_deref()
            .or(self.ui_token_amount.amount.as_deref())
            .unwrap_or("0");
        Decimal::from_str(raw).unwrap_or(Decimal::ZERO)
    }
}

fn decode(resp: RpcResponse) -> Result<TxInfo, PaymentVerifyError> {
    if let Some(e) = resp.error {
        return Err(PaymentVerifyError::Rpc(e.to_string()));
    }
    resp.result.ok_or(PaymentVerifyError::NotFound)
}

async fn post(body: &str, rpc_url: &str) -> Result<String, PaymentVerifyError> {
    let mut init = worker::RequestInit::new();
    init.method = worker::Method::Post;
    init.body = Some(body.to_owned().into());
    let req = worker::Request::new_with_init(rpc_url, &init)
        .map_err(|e| PaymentVerifyError::Rpc(e.to_string()))?;
    req.headers()
        .set("Content-Type", "application/json")
        .map_err(|e| PaymentVerifyError::Rpc(e.to_string()))?;
    let mut resp = worker::send::SendFuture::new(worker::Fetch::Request(req).send())
        .await
        .map_err(|e| PaymentVerifyError::Rpc(e.to_string()))?;
    let status = resp.status_code();
    let text = worker::send::SendFuture::new(resp.text())
        .await
        .map_err(|e| PaymentVerifyError::Rpc(e.to_string()))?;
    tracing::debug!(
        "rpc raw status={status} body={}",
        &text[..text.len().min(200)]
    );
    Ok(text)
}

#[async_trait]
impl PaymentVerifier for RpcPaymentVerifier {
    async fn verify_usdc_transfer(
        &self,
        signature: &str,
        recipient: &str,
        mint: &str,
        amount: Decimal,
    ) -> Result<VerifiedPayment, PaymentVerifyError> {
        let body = serde_json::to_string(&RpcRequest {
            jsonrpc: "2.0",
            id: RPC_ID,
            method: JSONRPC_METHOD_GET_TX,
            params: serde_json::json!([
                signature,
                {
                    "encoding": JSONRPC_ENCODING,
                    "commitment": JSONRPC_COMMITMENT,
                    "maxSupportedTransactionVersion": JSONRPC_MAX_TX_VERSION,
                }
            ]),
        })
        .map_err(|e| PaymentVerifyError::Rpc(e.to_string()))?;

        let text = post(&body, &self.rpc_url).await?;
        let resp: RpcResponse =
            serde_json::from_str(&text).map_err(|e| PaymentVerifyError::Rpc(e.to_string()))?;
        let tx = decode(resp)?;
        let Some(meta) = tx.meta else {
            return Err(PaymentVerifyError::Rpc("missing meta".to_owned()));
        };
        if meta.err.is_some() {
            return Err(PaymentVerifyError::Failed);
        }

        let mut pre = std::collections::HashMap::<u32, Decimal>::new();
        for b in &meta.pre_token_balances {
            pre.insert(b.account_index, b.amount());
        }
        let mut transferred = Decimal::ZERO;
        let mut saw_mint = false;
        let mut saw_recipient = false;
        for b in &meta.post_token_balances {
            if b.mint != mint {
                continue;
            }
            saw_mint = true;
            if b.owner.as_deref() != Some(recipient) {
                continue;
            }
            saw_recipient = true;
            let delta = b.amount() - pre.get(&b.account_index).copied().unwrap_or(Decimal::ZERO);
            if delta > Decimal::ZERO {
                transferred += delta;
            }
        }
        if !saw_mint {
            return Err(PaymentVerifyError::WrongMint);
        }
        if !saw_recipient {
            return Err(PaymentVerifyError::WrongRecipient);
        }
        if transferred < amount {
            return Err(PaymentVerifyError::WrongAmount);
        }
        Ok(VerifiedPayment {
            amount: transferred,
            slot: tx.slot,
        })
    }
}
