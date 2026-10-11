use rust_decimal::Decimal;
use uuid::Uuid;

use pos_backend::domain::entities::{
    Billing, BillingLine, BillingStatus, Currency, Invite, InviteStatus, Order, PaymentDestination,
    Shop, User, VatMode,
};
use pos_backend::domain::services::{
    DomainRuleError, accept_invite, billing_can_transition, can_change_role, can_invite,
    validate_order,
};
use pos_backend::domain::value_objects::{Chain, ChainAddress, Email, Money, Role, SplToken};

const SOL_ADDRESS: &str = "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR";
const BTC_ADDRESS: &str = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
const NOW: chrono::DateTime<chrono::Utc> = chrono::DateTime::UNIX_EPOCH;

fn shop_with_vat() -> Shop {
    Shop {
        id: Uuid::now_v7(),
        owner_id: Uuid::now_v7(),
        name: "test shop".to_owned(),
        vat_mode: VatMode::None,
        vat_rate: Decimal::ZERO,
        fx_thb_per_usd: Decimal::ZERO,
        currency: Currency::default(),
        deleted_at: None,
    }
}

fn email(raw: &str) -> Email {
    Email::parse(raw).expect("test email is valid")
}

fn money(amount: i64) -> Money {
    Money::new(Decimal::from(amount)).expect("test amount is valid")
}

fn line(price: i64, qty: u32) -> BillingLine {
    BillingLine {
        goods_id: Uuid::now_v7(),
        name: "widget".to_owned(),
        price: money(price),
        cost: Decimal::from(1),
        qty,
    }
}

fn sol_destination() -> PaymentDestination {
    PaymentDestination {
        id: Uuid::now_v7(),
        shop_id: Uuid::now_v7(),
        address: ChainAddress::parse(Chain::Solana, SOL_ADDRESS)
            .expect("test sol address is valid"),
        token: SplToken::default(),
    }
}

struct TestPromptPay;

impl pos_backend::port::promptpay::PromptPayEncoder for TestPromptPay {
    fn target(&self, raw: &str) -> String {
        raw.to_owned()
    }

    fn encode_payload(&self, target_id: &str, _amount: Decimal) -> String {
        target_id.to_owned()
    }
}

#[test]
pub fn pay_payload_encodes_spl_token_mint() {
    use rust_decimal::Decimal;

    let mut dest = sol_destination();
    let uri = pos_backend::app::service::billing::pay_payload(&TestPromptPay, &dest, Decimal::ONE);
    assert_eq!(
        uri.split("spl-token=").nth(1),
        Some("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
        "{uri}"
    );

    dest.token = SplToken::Usdt;
    let uri = pos_backend::app::service::billing::pay_payload(&TestPromptPay, &dest, Decimal::ONE);
    assert_eq!(
        uri.split("spl-token=").nth(1),
        Some("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"),
        "{uri}"
    );
}

#[test]
fn email_rejects_bad_shapes() {
    assert!(Email::parse("nope").is_err());
    assert!(Email::parse("@x.com").is_err());
    assert!(Email::parse("a@nodot").is_err());
    assert!(Email::parse("a b@x.com").is_err());
    assert!(Email::parse("user@example.com").is_ok());
}

#[test]
fn chain_addresses_validate_per_chain() {
    assert!(ChainAddress::parse(Chain::Solana, SOL_ADDRESS).is_ok());
    assert!(ChainAddress::parse(Chain::Solana, "short").is_err());
    assert!(ChainAddress::parse(Chain::Eth, "0x71C7656EC7ab88b098defB751B7401B5f6d8976F").is_ok());
    assert!(ChainAddress::parse(Chain::Eth, "71C7656EC7ab88b098defB751B7401B5f6d8976F").is_err());
    assert!(ChainAddress::parse(Chain::Btc, BTC_ADDRESS).is_ok());
    assert!(ChainAddress::parse(Chain::Btc, "xyz").is_err());
}

#[test]
fn role_hierarchy_gates_invites_and_role_changes() {
    assert!(can_invite(Role::Owner, Role::Admin).is_ok());
    assert!(can_invite(Role::Owner, Role::Owner).is_err());
    assert!(matches!(
        can_invite(Role::Employee, Role::Admin),
        Err(DomainRuleError::Forbidden)
    ));

    assert!(can_change_role(Role::Owner, Role::Admin, Role::Employee).is_ok());
    assert!(can_change_role(Role::Admin, Role::Owner, Role::Employee).is_err());
    assert!(can_change_role(Role::Owner, Role::Employee, Role::Owner).is_err());
}

#[test]
fn invite_accepts_only_when_pending_unexpired_and_email_matches() {
    let user = User {
        id: Uuid::now_v7(),
        email: Some(email("a@b.com")),
        wallet: None,
        role: Role::Employee,
        created_at: NOW,
    };
    let invite = Invite {
        id: Uuid::now_v7(),
        shop_id: Uuid::now_v7(),
        email: email("a@b.com"),
        role: Role::Employee,
        status: InviteStatus::Pending,
        expires_at: NOW + chrono::Duration::hours(24),
    };
    let member = accept_invite(&invite, &user, NOW).expect("invite is acceptable");
    assert_eq!(member.role, Role::Employee);

    let expired = Invite {
        expires_at: NOW - chrono::Duration::hours(1),
        ..invite.clone()
    };
    assert!(accept_invite(&expired, &user, NOW).is_err());

    let other_user = User {
        email: Some(email("c@d.com")),
        ..user
    };
    assert!(accept_invite(&invite, &other_user, NOW).is_err());

    // Wallet-only users (no linked email) accept by invite id capability.
    let wallet_user = User {
        email: None,
        wallet: None,
        id: Uuid::now_v7(),
        role: Role::Employee,
        created_at: NOW,
    };
    assert!(accept_invite(&invite, &wallet_user, NOW).is_ok());
}

#[test]
fn billing_total_sums_line_totals() {
    let billing = Billing {
        id: Uuid::now_v7(),
        shop_id: Uuid::now_v7(),
        status: BillingStatus::Pending,
        lines: vec![line(10, 2), line(5, 1)],
        destination: sol_destination(),
        vat_mode: VatMode::None,
        vat_rate: Decimal::ZERO,
    };
    assert_eq!(billing.total(), Decimal::from(25));
}

#[test]
fn order_validates_and_converts_to_pending_billing() {
    let destination = sol_destination();
    let order = Order {
        id: Uuid::now_v7(),
        shop_id: destination.shop_id,
        lines: vec![line(3, 3)],
        destination_id: destination.id,
        created_at: NOW,
    };
    assert_eq!(validate_order(&order).ok(), Some(Decimal::from(9)));

    let billing = order.to_billing(destination.clone(), &shop_with_vat());
    assert_eq!(billing.status, BillingStatus::Pending);
    assert_eq!(billing.destination.id, destination.id);

    let empty = Order {
        lines: Vec::new(),
        ..order
    };
    assert!(validate_order(&empty).is_err());
}

#[test]
fn billing_transitions_only_from_pending() {
    let base = Billing {
        id: Uuid::now_v7(),
        shop_id: Uuid::now_v7(),
        status: BillingStatus::Pending,
        lines: vec![line(1, 1)],
        destination: sol_destination(),
        vat_mode: VatMode::None,
        vat_rate: Decimal::ZERO,
    };
    assert!(billing_can_transition(&base, BillingStatus::Paid));
    assert!(billing_can_transition(&base, BillingStatus::Cancelled));
    let paid = Billing {
        status: BillingStatus::Paid,
        ..base
    };
    assert!(!billing_can_transition(&paid, BillingStatus::Cancelled));
}

#[test]
fn money_rejects_non_positive() {
    assert!(Money::new(Decimal::ZERO).is_err());
    assert!(Money::new(Decimal::from(-1)).is_err());
    assert!(Money::new(Decimal::from(1)).is_ok());
}
