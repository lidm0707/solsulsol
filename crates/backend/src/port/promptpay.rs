//! PromptPay payload encoding port.

use rust_decimal::Decimal;

pub trait PromptPayEncoder: Send + Sync {
    fn target(&self, raw: &str) -> String;
    fn encode_payload(&self, target_id: &str, amount: Decimal) -> String;
}
