//! Entities: identity + lifecycle, equality by id.

use crate::domain::value_objects::{ChainAddress, Email, Money, PasswordHash, Role, SplToken};
use chrono::DateTime;
use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub email: Email,
    pub password: PasswordHash,
    pub role: Role,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Shop {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub vat_mode: VatMode,
    pub vat_rate: Decimal,
}

#[derive(Debug, Clone)]
pub struct ShopMember {
    pub shop_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
}

#[derive(Debug, Clone)]
pub struct Product {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub name: String,
    pub price: Money,
    pub cost: rust_decimal::Decimal,
    pub units: i64,
    pub barcode: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PaymentDestination {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub address: ChainAddress,
    pub token: SplToken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteStatus {
    Pending,
    Accepted,
    Revoked,
}

impl InviteStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Revoked => "revoked",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "pending" => Some(Self::Pending),
            "accepted" => Some(Self::Accepted),
            "revoked" => Some(Self::Revoked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Invite {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub email: Email,
    pub role: Role,
    pub status: InviteStatus,
    pub expires_at: DateTime<Utc>,
}

impl Invite {
    pub fn is_acceptable(&self, now: DateTime<Utc>) -> bool {
        self.status == InviteStatus::Pending && now < self.expires_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvoiceStatus {
    Pending,
    Paid,
    Cancelled,
}

impl InvoiceStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Paid => "paid",
            Self::Cancelled => "cancelled",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "pending" => Some(Self::Pending),
            "paid" => Some(Self::Paid),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

/// VAT treatment configured per shop.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VatMode {
    /// No VAT is shown or charged.
    #[default]
    None,
    /// VAT is added on top of line prices.
    Exclusive,
    /// Line prices already contain VAT; it is extracted for display.
    Inclusive,
}

impl VatMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Exclusive => "exclusive",
            Self::Inclusive => "inclusive",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "none" => Some(Self::None),
            "exclusive" => Some(Self::Exclusive),
            "inclusive" => Some(Self::Inclusive),
            _ => None,
        }
    }
}

pub const VAT_RATE_MAX: Decimal = Decimal::from_parts(100, 0, 0, false, 0);

fn round2(v: Decimal) -> Decimal {
    v.round_dp(2)
}

#[derive(Debug, Clone)]
pub struct Invoice {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub status: InvoiceStatus,
    pub lines: Vec<InvoiceLine>,
    pub destination: PaymentDestination,
    pub vat_mode: VatMode,
    pub vat_rate: Decimal,
}

impl Invoice {
    /// Lines total before VAT (equals gross when VAT is inclusive).
    pub fn subtotal(&self) -> Decimal {
        InvoiceLine::total_amount(self.lines.iter())
    }

    pub fn vat_amount(&self) -> Decimal {
        match self.vat_mode {
            VatMode::None => Decimal::ZERO,
            VatMode::Exclusive => round2(self.subtotal() * self.vat_rate / VAT_RATE_MAX),
            VatMode::Inclusive => {
                round2(self.subtotal() * self.vat_rate / (VAT_RATE_MAX + self.vat_rate))
            }
        }
    }

    /// Gross amount the customer pays (VAT added on top when exclusive).
    pub fn total(&self) -> Decimal {
        match self.vat_mode {
            VatMode::Exclusive => self.subtotal() + self.vat_amount(),
            _ => self.subtotal(),
        }
    }

    pub fn cost(&self) -> Decimal {
        self.lines
            .iter()
            .fold(Decimal::ZERO, |acc, l| acc + l.cost * Decimal::from(l.qty))
    }
}

#[derive(Debug, Clone)]
pub struct InvoiceLine {
    pub product_id: Uuid,
    pub name: String,
    pub price: Money,
    pub cost: Decimal,
    pub qty: u32,
}

impl InvoiceLine {
    fn line_total(&self) -> Decimal {
        self.price.amount() * Decimal::from(self.qty)
    }

    pub fn total_amount<'a>(lines: impl Iterator<Item = &'a Self>) -> Decimal {
        lines.fold(Decimal::ZERO, |acc, l| acc + l.line_total())
    }
}

/// A checkout order: goods selected + chosen payment destination.
#[derive(Debug, Clone)]
pub struct Order {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub lines: Vec<InvoiceLine>,
    pub destination_id: Uuid,
    pub created_at: DateTime<Utc>,
}

impl Order {
    pub fn total(&self) -> Decimal {
        InvoiceLine::total_amount(self.lines.iter())
    }

    pub fn to_invoice(&self, destination: PaymentDestination, shop: &Shop) -> Invoice {
        Invoice {
            id: self.id,
            shop_id: self.shop_id,
            status: InvoiceStatus::Pending,
            lines: self.lines.clone(),
            destination,
            vat_mode: shop.vat_mode,
            vat_rate: shop.vat_rate,
        }
    }
}
