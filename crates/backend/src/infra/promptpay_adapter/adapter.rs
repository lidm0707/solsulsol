//! PromptPay adapter: implements pos-backend PromptPayEncoder with pos-promptpay.

use crate::port::promptpay::PromptPayEncoder;
use rust_decimal::Decimal;

pub struct PromptPayAdapter;

impl PromptPayEncoder for PromptPayAdapter {
    fn target(&self, raw: &str) -> String {
        pos_promptpay::target::target(raw)
    }

    fn encode_payload(&self, target_id: &str, amount: Decimal) -> String {
        pos_promptpay::payload::build_payload(target_id, amount)
    }
}
