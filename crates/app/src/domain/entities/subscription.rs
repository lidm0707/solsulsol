//! Subscription, payment order and auth nonce entities + package limits.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

pub const SUB_PRICE_USDC: Decimal = Decimal::from_parts(1, 0, 0, false, 0);
pub const MAX_SHOPS_PER_USER: usize = 1;
pub const MAX_GOODS_PER_SHOP: usize = 30;
pub const MAX_STORAGE_BYTES: i64 = 10 * 1024 * 1024;
pub const USDC_DECIMALS: u32 = 6;

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
