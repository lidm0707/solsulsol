//! PDF billing tests: real QR → PDF, garbage QR rejected.

use rust_decimal::Decimal;
use uuid::Uuid;

use pos_backend::app::service::pdf::render_billing_pdf;
use pos_backend::domain::entities::{
    Billing, BillingLine, BillingStatus, Currency, PaymentDestination, VatMode,
};
use pos_backend::domain::value_objects::{Chain, ChainAddress, Money, SplToken};
use pos_qr_code::encode::encode_png;

const SOL: &str = "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR";

fn billing() -> Billing {
    Billing {
        id: Uuid::now_v7(),
        shop_id: Uuid::now_v7(),
        status: BillingStatus::Pending,
        lines: vec![BillingLine {
            goods_id: Uuid::now_v7(),
            name: "coffee".to_owned(),
            price: Money::new(Decimal::from(4)).expect("test amount is valid"),
            cost: Decimal::from(1),
            qty: 2,
        }],
        destination: PaymentDestination {
            id: Uuid::now_v7(),
            shop_id: Uuid::now_v7(),
            address: ChainAddress::parse(Chain::Solana, SOL).expect("test sol address is valid"),
            token: SplToken::default(),
        },
        vat_mode: VatMode::Exclusive,
        vat_rate: Decimal::new(1000, 2),
    }
}

#[test]
fn renders_pdf_with_embedded_qr() {
    let qr = encode_png(&format!("pay:sol:{SOL}")).expect("payload encodes");
    let pdf = render_billing_pdf(&billing(), "Test Shop", &qr, Currency::Thb).expect("pdf renders");
    assert!(pdf.starts_with(b"%PDF"), "PDF magic");
    assert!(pdf.len() > 1000, "includes embedded QR image");
}

#[test]
fn rejects_garbage_qr_bytes() {
    assert!(render_billing_pdf(&billing(), "Test Shop", b"not a png", Currency::Thb).is_err());
}

// Note: content streams are compressed by lopdf, so rendered text is not
// greppable in the raw bytes; coverage = valid PDF magic + embedded QR + error path.
