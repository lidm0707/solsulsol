//! Storage ports.

use crate::domain::entities::subscription::{
    AuthNonce, PaymentOrder, Subscription, SubscriptionStatus,
};
use crate::domain::entities::{
    Billing, Currency, Goods, Invite, InviteStatus, PaymentDestination, Shop, ShopMember, User,
    VatMode,
};
use crate::domain::value_objects::Role;
use async_trait::async_trait;
use rust_decimal::Decimal;
use uuid::Uuid;

#[async_trait]
pub trait UserRepo: Send + Sync {
    async fn insert(&self, user: &User) -> Result<(), String>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, String>;
    async fn find_by_wallet(&self, wallet: &str) -> Result<Option<User>, String>;
}

#[async_trait]
pub trait NonceRepo: Send + Sync {
    async fn insert(&self, nonce: &AuthNonce) -> Result<(), String>;
    async fn find(&self, wallet: &str, nonce: &str) -> Result<Option<AuthNonce>, String>;
    /// Atomically marks the nonce used; `Ok(false)` when missing, already
    /// used, expired or owned by another wallet (replay / expiry rejected).
    async fn consume(
        &self,
        wallet: &str,
        nonce: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<bool, String>;
}

#[async_trait]
pub trait SubscriptionRepo: Send + Sync {
    async fn find_by_user(&self, user_id: Uuid) -> Result<Option<Subscription>, String>;
    async fn upsert(&self, sub: &Subscription) -> Result<(), String>;
    /// Active subs whose `expires_at` has passed; candidates for grace.
    async fn list_to_mark_grace(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Subscription>, String>;
    /// Grace subs past `grace_until`; candidates for soft deletion.
    async fn list_to_mark_lapsed(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Subscription>, String>;
    /// Lapsed or already-Deleted subs past the recovery window; candidates
    /// for hard deletion (Deleted included so the shop purge still runs when
    /// the sub was marked Deleted before its shops became purge-eligible).
    async fn list_to_delete(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Subscription>, String>;
    async fn set_status(
        &self,
        id: Uuid,
        status: SubscriptionStatus,
        grace_until: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), String>;
}

#[async_trait]
pub trait PaymentOrderRepo: Send + Sync {
    async fn insert(&self, order: &PaymentOrder) -> Result<(), String>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<PaymentOrder>, String>;
    async fn find_by_signature(&self, signature: &str) -> Result<Option<PaymentOrder>, String>;
    /// Atomic guard: flips pending → paid only once. `Ok(false)` when the
    /// order was already paid (duplicate signature / concurrent verify).
    async fn mark_paid_if_pending(
        &self,
        id: Uuid,
        signature: &str,
        paid_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<bool, String>;
}

#[async_trait]
pub trait StorageUsageRepo: Send + Sync {
    /// Records `bytes` under a unique key (idempotent per key) unless the
    /// shop total would exceed `quota`; returns the new total.
    async fn add_checked(
        &self,
        shop_id: Uuid,
        key: Uuid,
        bytes: i64,
        quota: i64,
    ) -> Result<i64, StorageError>;
    async fn total(&self, shop_id: Uuid) -> Result<i64, StorageError>;
    /// `(key, bytes)` per recorded object, ascending by key.
    async fn list(&self, shop_id: Uuid) -> Result<Vec<(Uuid, i64)>, StorageError>;
    /// Removes the usage row; returns the shop total after removal.
    async fn remove(&self, shop_id: Uuid, key: Uuid) -> Result<i64, StorageError>;
}

#[derive(Debug)]
pub enum StorageError {
    QuotaExceeded,
    Storage(String),
}

impl From<String> for StorageError {
    fn from(e: String) -> Self {
        StorageError::Storage(e)
    }
}

#[async_trait]
pub trait ShopRepo: Send + Sync {
    async fn insert(&self, shop: &Shop) -> Result<(), String>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Shop>, String>;
    async fn find_for_user(&self, user_id: Uuid) -> Result<Vec<Shop>, String>;

    /// Persist shop-level sale settings (VAT). Admin-gated in the service layer.
    async fn update_settings(
        &self,
        _id: Uuid,
        _vat_mode: VatMode,
        _vat_rate: Decimal,
        _fx_thb_per_usd: Decimal,
        _currency: Currency,
    ) -> Result<(), String> {
        Err("shop settings not supported".into())
    }

    /// Marks every shop owned by `owner_id` deleted at `now` (soft).
    async fn soft_delete_for_owner(
        &self,
        _owner_id: Uuid,
        _now: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, String> {
        Err("shop soft delete not supported".into())
    }

    /// Permanently removes shops (and cascades) soft-deleted before `before`.
    async fn hard_delete_expired(
        &self,
        _before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, String> {
        Err("shop hard delete not supported".into())
    }
}

#[async_trait]
pub trait ShopMemberRepo: Send + Sync {
    async fn insert(&self, member: &ShopMember) -> Result<(), String>;
    async fn find_role(&self, shop_id: Uuid, user_id: Uuid) -> Result<Option<Role>, String>;
    async fn list_by_shop(&self, shop_id: Uuid) -> Result<Vec<ShopMember>, String>;
    async fn update_role(&self, shop_id: Uuid, user_id: Uuid, role: Role) -> Result<(), String>;
}

#[async_trait]
pub trait GoodsRepo: Send + Sync {
    async fn insert(&self, goods: &Goods) -> Result<(), String>;
    async fn find_by_shop(&self, shop_id: Uuid) -> Result<Vec<Goods>, String>;
    async fn find_by_id(&self, goods_id: Uuid) -> Result<Option<Goods>, String>;
    async fn update(&self, goods: &Goods) -> Result<(), String>;
    async fn delete(&self, goods_id: Uuid) -> Result<(), String>;
    /// Removes one unit from stock; errors when the goods is out of stock.
    async fn take_units(&self, goods_id: Uuid) -> Result<(), String>;
}

#[async_trait]
pub trait DestinationRepo: Send + Sync {
    async fn insert(&self, destination: &PaymentDestination) -> Result<(), String>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<PaymentDestination>, String>;
    async fn list_by_shop(&self, shop_id: Uuid) -> Result<Vec<PaymentDestination>, String>;
    async fn delete(&self, id: Uuid) -> Result<(), String>;
}

#[async_trait]
pub trait InviteRepo: Send + Sync {
    async fn insert(&self, invite: &Invite) -> Result<(), String>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Invite>, String>;
    async fn list_by_shop(&self, shop_id: Uuid) -> Result<Vec<Invite>, String>;
    async fn set_status(&self, id: Uuid, status: InviteStatus) -> Result<(), String>;
}

#[async_trait]
pub trait BillingRepo: Send + Sync {
    async fn insert(&self, billing: &Billing) -> Result<(), String>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Billing>, String>;
    async fn list_by_shop(&self, shop_id: Uuid) -> Result<Vec<Billing>, String>;
    /// Sold quantity per goods (paid billings only), best-sell first source.
    async fn sales_by_goods(&self, shop_id: Uuid) -> Result<Vec<(Uuid, u64)>, String>;
    /// Whether any billing line (paid or not) references the goods.
    async fn goods_has_lines(&self, goods_id: Uuid) -> Result<bool, String>;
    /// Deletes billings of shops soft-deleted before `before`. Runs before the
    /// shops purge — billings have no ON DELETE CASCADE on shop_id.
    async fn hard_delete_expired(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, String>;
}

/// Transaction port: services open scoped transactions and commit/rollback.
#[async_trait]
pub trait Tx: Send + Sync {
    async fn commit(self: Box<Self>) -> Result<(), String>;
    async fn rollback(self: Box<Self>) -> Result<(), String>;
}

#[async_trait]
pub trait TxManager: Send + Sync {
    async fn begin(&self) -> Result<Box<dyn Tx>, String>;
}
