//! Invoice use-case: select goods, attach payment destination, render QR, store invoice.

use crate::domain::entities::{Invoice, InvoiceLine, Order, PaymentDestination, Shop};
use crate::domain::value_objects::{Chain, new_id};
use rust_decimal::Decimal;

use crate::port::{PromptPayEncoder, QrRenderer};
use crate::service::use_cases::AppServices;

#[cfg(not(target_arch = "wasm32"))]
const POS_SOLANA_CLUSTER_ENV: &str = "POS_SOLANA_CLUSTER";

#[derive(Debug)]
pub enum InvoiceError {
    NoProducts,
    Storage(String),
}

impl From<String> for InvoiceError {
    fn from(e: String) -> Self {
        InvoiceError::Storage(e)
    }
}

pub async fn checkout(
    svc: &AppServices,
    qr: &dyn QrRenderer,
    pp: &dyn PromptPayEncoder,
    shop: &Shop,
    product_ids: &[uuid::Uuid],
    destination: PaymentDestination,
) -> Result<(Invoice, Vec<u8>), InvoiceError> {
    let shop_id = shop.id;
    let all = svc.products.find_by_shop(shop_id).await?;
    let mut lines = Vec::new();
    for id in product_ids {
        let Some(p) = all.iter().find(|p| p.id == *id) else {
            return Err(InvoiceError::Storage("product not in shop".into()));
        };
        if p.units > 0 {
            svc.products.take_units(p.id).await?;
        }
        lines.push(InvoiceLine {
            product_id: p.id,
            name: p.name.clone(),
            price: p.price,
            cost: p.cost,
            qty: 1,
        });
    }
    if lines.is_empty() {
        return Err(InvoiceError::NoProducts);
    }
    let order = Order {
        id: new_id(),
        shop_id,
        lines,
        destination_id: destination.id,
        created_at: chrono::Utc::now(),
    };
    crate::domain::services::validate_order(&order)
        .map_err(|e| InvoiceError::Storage(e.to_string()))?;
    let invoice = order.to_invoice(destination, shop);
    let payload = pay_payload(pp, &invoice.destination, invoice.total());
    let qr_png = qr.render_png(&payload).await?;
    svc.invoices.insert(&invoice).await?;
    tracing::info!(invoice_id = %invoice.id, total = %invoice.total(), "invoice created");
    Ok((invoice, qr_png))
}

pub fn pay_payload(pp: &dyn PromptPayEncoder, dest: &PaymentDestination, total: Decimal) -> String {
    match dest.address.chain() {
        // Solana Pay transfer URI — wallet apps prefill the amount when scanned.
        Chain::Solana => {
            let token = match dest.token.mint() {
                Some(mint) => format!("&spl-token={mint}"),
                None => String::new(),
            };
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
                total.normalize(),
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
            pp.encode_payload(&target_id, total)
        }
        chain => format!(
            "pay:{}:{}:{}",
            chain_label(chain),
            dest.address.as_str(),
            dest.id
        ),
    }
}

fn chain_label(chain: Chain) -> &'static str {
    match chain {
        Chain::Solana => "sol",
        Chain::Eth => "eth",
        Chain::Btc => "btc",
        Chain::Binance => "bnb",
        Chain::Bank => "bank",
    }
}
