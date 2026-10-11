//! Subscription, payment order and auth nonce entities + package limits.

use chrono::{DateTime, Duration, Months, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

pub const SUB_PRICE_USDC: Decimal = Decimal::from_parts(1, 0, 0, false, 0);
pub const SUB_PRICE_YEARLY_USDC: Decimal = Decimal::from_parts(10, 0, 0, false, 0);
pub const MAX_SHOPS_PER_USER: usize = 1;
pub const MAX_GOODS_PER_SHOP: usize = 30;
pub const MAX_STORAGE_BYTES: i64 = 10 * 1024 * 1024;
pub const USDC_DECIMALS: u32 = 6;

/// Billing plans. The order amount is the plan identity: orders are created
/// server-side with an exact price, and the paid amount selects the period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingPeriod {
    Monthly,
    Yearly,
}

impl BillingPeriod {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Monthly => "monthly",
            Self::Yearly => "yearly",
        }
    }

    #[must_use]
    pub const fn price(self) -> Decimal {
        match self {
            Self::Monthly => SUB_PRICE_USDC,
            Self::Yearly => SUB_PRICE_YEARLY_USDC,
        }
    }

    #[must_use]
    pub fn from_price(amount: Decimal) -> Option<Self> {
        if amount == SUB_PRICE_USDC {
            Some(Self::Monthly)
        } else if amount == SUB_PRICE_YEARLY_USDC {
            Some(Self::Yearly)
        } else {
            None
        }
    }

    /// Extends an expiry by the bought period (renewals stack on remaining
    /// time).
    #[must_use]
    pub fn advance(self, from: DateTime<Utc>) -> DateTime<Utc> {
        match self {
            Self::Monthly => from.checked_add_months(Months::new(1)),
            Self::Yearly => from.checked_add_months(Months::new(12)),
        }
        .unwrap_or(from + Duration::days(30))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionStatus {
    Active,
    /// Expired; data retained during grace, restricted actions blocked.
    Grace,
    /// Grace ended; shops soft-deleted, pending irreversible deletion.
    Lapsed,
    /// Data permanently deleted.
    Deleted,
}

impl SubscriptionStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Grace => "grace",
            Self::Lapsed => "lapsed",
            Self::Deleted => "deleted",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "active" => Some(Self::Active),
            "grace" => Some(Self::Grace),
            "lapsed" => Some(Self::Lapsed),
            "deleted" => Some(Self::Deleted),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Subscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub status: SubscriptionStatus,
    pub started_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub grace_until: Option<DateTime<Utc>>,
}

impl Subscription {
    #[must_use]
    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        self.status == SubscriptionStatus::Active && now < self.expires_at
    }

    #[must_use]
    pub fn in_grace(&self, now: DateTime<Utc>) -> bool {
        self.status == SubscriptionStatus::Grace && self.grace_until.is_some_and(|g| now < g)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentOrderStatus {
    Pending,
    Paid,
}

impl PaymentOrderStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Paid => "paid",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "pending" => Some(Self::Pending),
            "paid" => Some(Self::Paid),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PaymentOrder {
    pub id: Uuid,
    pub user_id: Uuid,
    pub amount: Decimal,
    pub mint: String,
    pub recipient: String,
    pub status: PaymentOrderStatus,
    pub signature: Option<String>,
    pub created_at: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct AuthNonce {
    pub id: Uuid,
    pub wallet: String,
    pub nonce: String,
    pub expires_at: DateTime<Utc>,
}
