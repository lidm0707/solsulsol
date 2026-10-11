//! Billing use-case: select goods, attach payment destination, render QR, store billing.

use crate::domain::entities::Currency;
use crate::domain::entities::{Billing, BillingLine, Order, PaymentDestination, Shop, round2};
use crate::domain::value_objects::{Chain, new_id};
use rust_decimal::Decimal;

use crate::port::promptpay::PromptPayEncoder;
use crate::port::qr::QrRenderer;
use crate::service::use_cases::AppServices;

#[cfg(not(target_arch = "wasm32"))]
const POS_SOLANA_CLUSTER_ENV: &str = "POS_SOLANA_CLUSTER";

#[derive(Debug)]
pub enum BillingError {
    NoGoods,
    Subscription(crate::service::subscription::SubError),
    Storage(String),
}

impl From<String> for BillingError {
    fn from(e: String) -> Self {
        BillingError::Storage(e)
    }
}

pub async fn checkout(
    svc: &AppServices,
    actor_id: uuid::Uuid,
    qr: &dyn QrRenderer,
    pp: &dyn PromptPayEncoder,
    shop: &Shop,
    goods_lines: &[(uuid::Uuid, u32)],
    destination: PaymentDestination,
) -> Result<(Billing, Vec<u8>), BillingError> {
    crate::service::subscription::require_active(svc, actor_id)
        .await
        .map_err(BillingError::Subscription)?;
    let shop_id = shop.id;
    let all = svc.goods.find_by_shop(shop_id).await?;
    let mut lines = Vec::new();
    for (id, qty) in goods_lines {
        let Some(p) = all.iter().find(|p| p.id == *id) else {
            return Err(BillingError::Storage("goods not in shop".into()));
        };
        if *qty == 0 {
            continue;
        }
        if p.units > 0 {
            for _ in 0..*qty {
                svc.goods.take_units(p.id).await?;
            }
        }
        lines.push(BillingLine {
            goods_id: p.id,
            name: p.name.clone(),
            price: p.price,
            cost: p.cost,
            qty: *qty,
        });
    }
    if lines.is_empty() {
        return Err(BillingError::NoGoods);
    }
    let order = Order {
        id: new_id(),
        shop_id,
        lines,
        destination_id: destination.id,
        created_at: chrono::Utc::now(),
    };
    crate::domain::services::validate_order(&order)
        .map_err(|e| BillingError::Storage(e.to_string()))?;
    let billing = order.to_billing(destination, shop);
    let payload = pay_payload(pp, &billing.destination, billing.total());
    let qr_png = qr.render_png(&payload).await?;
    svc.billings.insert(&billing).await?;
    tracing::info!(billing_id = %billing.id, total = %billing.total(), "billing created");
    Ok((billing, qr_png))
}

/// Amount encoded in the QR payload, converted from the shop pricing
/// currency: PromptPay mandates THB (BOT standard), Solana Pay carries the
/// USD-pegged token amount.
const TOKEN_AMOUNT_DP: u32 = 5;

pub fn qr_amount(total: Decimal, chain: Chain, currency: Currency, fx: Decimal) -> Decimal {
    match chain {
        Chain::Bank if currency == Currency::Usdt && fx > Decimal::ZERO => round2(total * fx),
        Chain::Solana if currency == Currency::Thb && fx > Decimal::ZERO => {
            (total / fx).round_dp(TOKEN_AMOUNT_DP).normalize()
        }
        _ => total,
    }
}

pub fn pay_payload(
    pp: &dyn PromptPayEncoder,
    dest: &PaymentDestination,
    amount: Decimal,
) -> String {
    match dest.address.chain() {
        // Solana Pay transfer URI — wallet apps prefill the amount when scanned.
        Chain::Solana => {
            let token = format!("&spl-token={}", dest.token.mint());
            // cluster param routes wallet apps to a non-mainnet network (test mode);
            // std::env is unimplemented on wasm32 (workers), so only read it natively
            #[cfg(target_arch = "wasm32")]
            let cluster = String::new();
            #[cfg(not(target_arch = "wasm32"))]
            let cluster = match std::env::var(POS_SOLANA_CLUSTER_ENV).as_deref() {
                Ok(c) if !c.is_empty() => format!("&cluster={c}"),
                _ => String::new(),
            };
            format!(
                "solana:{}?amount={}&message={}{}{}",
                dest.address.as_str(),
                amount.normalize(),
                dest.id,
                token,
                cluster
            )
        }
        Chain::Bank => {
            let target_id = pp.target(dest.address.as_str());
            if target_id.is_empty() {
                return format!(
                    "pay:{}:{}:{}",
                    chain_label(dest.address.chain()),
                    dest.address.as_str(),
                    dest.id
                );
            }
            pp.encode_payload(&target_id, amount)
        }
    }
}

fn chain_label(chain: Chain) -> &'static str {
    match chain {
        Chain::Solana => "sol",
        Chain::Bank => "bank",
    }
}
