//! Solana JSON-RPC payment verification: getTransaction + token balance
//! deltas. Confirms status, recipient, mint, amount and finality.

use pos_app::port::solana::{PaymentVerifier, PaymentVerifyError, VerifiedPayment};
use rust_decimal::Decimal;
use serde_json::Value;

pub struct RpcPaymentVerifier {
    pub rpc_url: String,
    pub client: reqwest::Client,
}

const COMMITMENT_FINALIZED: &str = "finalized";
const USDC_DECIMALS: u32 = 6;
const JSONRPC_VERSION: &str = "2.0";
const GET_TRANSACTION_METHOD: &str = "getTransaction";
const RPC_TIMEOUT_SECS: u64 = 30;
const REQ_ID: i32 = 1;

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TokenBalance {
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    mint: Option<String>,
    account_index: u32,
    #[serde(default)]
    ui_token_amount: Option<UiTokenAmount>,
}

#[derive(serde::Deserialize, Debug)]
struct UiTokenAmount {
    #[serde(default)]
    amount: String,
    #[serde(default)]
    decimals: u32,
}

impl RpcPaymentVerifier {
    pub fn new(rpc_url: &str) -> Self {
        Self {
            rpc_url: rpc_url.to_owned(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(RPC_TIMEOUT_SECS))
                .build()
                .unwrap_or_default(),
        }
    }

    async fn rpc(&self, signature: &str) -> Result<Option<Value>, PaymentVerifyError> {
        let body = serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": REQ_ID,
            "method": GET_TRANSACTION_METHOD,
            "params": [
                signature,
                {
                    "encoding": "jsonParsed",
                    "commitment": COMMITMENT_FINALIZED,
                    "maxSupportedTransactionVersion": 0,
                }
            ]
        });
        let resp = self
            .client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| PaymentVerifyError::Rpc(e.to_string()))?;
        let json: Value = resp
            .json()
            .await
            .map_err(|e| PaymentVerifyError::Rpc(e.to_string()))?;
        if let Some(err) = json.get("error") {
            return Err(PaymentVerifyError::Rpc(err.to_string()));
        }
        Ok(json.get("result").filter(|r| !r.is_null()).cloned())
    }
}

#[async_trait::async_trait]
impl PaymentVerifier for RpcPaymentVerifier {
    async fn verify_usdc_transfer(
        &self,
        signature: &str,
        recipient: &str,
        mint: &str,
        amount: Decimal,
    ) -> Result<VerifiedPayment, PaymentVerifyError> {
        let Some(tx) = self.rpc(signature).await? else {
            return Err(PaymentVerifyError::NotFound);
        };
        let meta = tx.get("meta").ok_or(PaymentVerifyError::Failed)?;
        if meta.get("err").is_some_and(|e| !e.is_null()) {
            return Err(PaymentVerifyError::Failed);
        }
        let slot = meta.get("slot").and_then(Value::as_i64).unwrap_or_default();
        let pre = token_balances(meta, "preTokenBalances");
        let post = token_balances(meta, "postTokenBalances");
        let mut credited = Decimal::ZERO;
        let mut saw_mint = false;
        for after in &post {
            if after.mint.as_deref() != Some(mint) {
                continue;
            }
            saw_mint = true;
            if after.owner.as_deref() != Some(recipient) {
                continue;
            }
            let before = pre
                .iter()
                .find(|b| b.account_index == after.account_index)
                .and_then(|b| b.raw_amount())
                .unwrap_or(0u128);
            let delta = after
                .raw_amount()
                .and_then(|a| a.checked_sub(before))
                .unwrap_or(0u128);
            credited += to_amount(delta, after.decimals());
        }
        if !saw_mint {
            tracing::debug!(?post, "verify: mint not present in postTokenBalances");
            return Err(PaymentVerifyError::WrongMint);
        }
        if credited == Decimal::ZERO {
            tracing::debug!(%recipient, %mint, ?post, "verify: no credited delta to recipient");
            return Err(PaymentVerifyError::WrongRecipient);
        }
        if credited < amount {
            return Err(PaymentVerifyError::WrongAmount);
        }
        Ok(VerifiedPayment {
            amount: credited,
            slot,
        })
    }
}

impl TokenBalance {
    fn raw_amount(&self) -> Option<u128> {
        self.ui_token_amount.as_ref()?.amount.parse().ok()
    }

    fn decimals(&self) -> u32 {
        self.ui_token_amount
            .as_ref()
            .map_or(USDC_DECIMALS, |a| a.decimals)
    }
}

fn to_amount(raw: u128, decimals: u32) -> Decimal {
    Decimal::from(raw) / Decimal::from(10u32.pow(decimals))
}

fn token_balances(meta: &Value, key: &str) -> Vec<TokenBalance> {
    meta.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}
