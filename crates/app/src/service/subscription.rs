//! Subscriptions: USDC payment orders, backend verification, limits,
//! activation/renewal. Frontend values are never trusted for money.

use crate::config::PaymentConfig;
use crate::domain::entities::subscription::{
    BillingPeriod, MAX_SHOPS_PER_USER, PaymentOrder, PaymentOrderStatus, Subscription,
    SubscriptionStatus, USDC_DECIMALS,
};
use crate::domain::value_objects::new_id;
use crate::port::database::StorageError;
use crate::port::solana::{PaymentVerifier, PaymentVerifyError};
use crate::service::use_cases::AppServices;
use chrono::{DateTime, Months, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug)]
pub enum SubError {
    NoSubscription,
    Expired,
    QuotaShops,
    QuotaGoods,
    QuotaStorage,
    OrderNotFound,
    OrderAlreadyPaid,
    Verify(PaymentVerifyError),
    Storage(String),
}

impl From<String> for SubError {
    fn from(e: String) -> Self {
        SubError::Storage(e)
    }
}

impl From<StorageError> for SubError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::QuotaExceeded => SubError::QuotaStorage,
            StorageError::Storage(e) => SubError::Storage(e),
        }
    }
}

/// Subscription period: one calendar month from activation (or from the
/// previous expiry when renewing early — time is never lost).
#[must_use]
pub fn add_month(from: DateTime<Utc>) -> DateTime<Utc> {
    from.checked_add_months(Months::new(1))
        .unwrap_or(from + chrono::Duration::days(30))
}

/// Activation or renewal start: renewals stack on remaining time.
#[must_use]
pub fn renewal_base(current: Option<Subscription>, now: DateTime<Utc>) -> DateTime<Utc> {
    match current {
        Some(s) if s.expires_at > now => s.expires_at,
        _ => now,
    }
}

#[must_use]
pub fn build_order(
    cfg: &PaymentConfig,
    user_id: Uuid,
    period: BillingPeriod,
    now: DateTime<Utc>,
) -> PaymentOrder {
    PaymentOrder {
        id: new_id(),
        user_id,
        amount: period.price(),
        mint: cfg.usdc_mint.clone(),
        recipient: cfg.recipient.clone(),
        status: PaymentOrderStatus::Pending,
        signature: None,
        created_at: now,
        paid_at: None,
    }
}

/// Creates a fresh payment order from server-side config (price, mint,
/// recipient). Client-supplied amounts are ignored by design.
pub async fn create_order(
    svc: &AppServices,
    user_id: Uuid,
    period: BillingPeriod,
) -> Result<PaymentOrder, SubError> {
    let order = build_order(&svc.payment_config, user_id, period, Utc::now());
    svc.orders.insert(&order).await?;
    tracing::info!(user_id = %user_id, order_id = %order.id, "payment order created");
    Ok(order)
}

/// Verifies `signature` on chain, atomically marks the order paid (guarding
/// duplicates and concurrent verifies) and activates/extends the subscription.
pub async fn verify_order(
    svc: &AppServices,
    verifier: &dyn PaymentVerifier,
    user_id: Uuid,
    order_id: Uuid,
    signature: &str,
) -> Result<Subscription, SubError> {
    let order = svc
        .orders
        .find_by_id(order_id)
        .await?
        .ok_or(SubError::OrderNotFound)?;
    if order.user_id != user_id {
        return Err(SubError::OrderNotFound);
    }
    if order.status != PaymentOrderStatus::Pending {
        return Err(SubError::OrderAlreadyPaid);
    }
    if signature.trim().is_empty() {
        return Err(SubError::Verify(PaymentVerifyError::NotFound));
    }
    if svc
        .orders
        .find_by_signature(signature.trim())
        .await?
        .is_some_and(|o| o.id != order.id)
    {
        return Err(SubError::OrderAlreadyPaid);
    }
    verifier
        .verify_usdc_transfer(
            signature.trim(),
            &order.recipient,
            &order.mint,
            order.amount,
        )
        .await
        .map_err(SubError::Verify)?;
    // Atomic pending → paid; `Ok(false)` means a concurrent verify won.
    let marked = svc
        .orders
        .mark_paid_if_pending(order.id, signature.trim(), Utc::now())
        .await?;
    if !marked {
        return Err(SubError::OrderAlreadyPaid);
    }
    let sub = activate(svc, user_id, Utc::now(), period_from_order(&order)).await?;
    tracing::info!(user_id = %user_id, order_id = %order.id, "payment verified, subscription active");
    Ok(sub)
}

/// The plan an order buys; legacy orders predate yearly and are monthly.
#[must_use]
fn period_from_order(order: &PaymentOrder) -> BillingPeriod {
    BillingPeriod::from_price(order.amount).unwrap_or(BillingPeriod::Monthly)
}

/// Activates a new subscription or extends the existing one by the bought
/// period.
async fn activate(
    svc: &AppServices,
    user_id: Uuid,
    now: DateTime<Utc>,
    period: BillingPeriod,
) -> Result<Subscription, SubError> {
    let current = svc.subscriptions.find_by_user(user_id).await?;
    let start = renewal_base(current.clone(), now);
    let expires = period.advance(start);
    let sub = match current {
        Some(s) => Subscription {
            status: SubscriptionStatus::Active,
            started_at: if s.status == SubscriptionStatus::Active {
                s.started_at
            } else {
                now
            },
            expires_at: expires,
            grace_until: None,
            ..s
        },
        None => Subscription {
            id: new_id(),
            user_id,
            status: SubscriptionStatus::Active,
            started_at: now,
            expires_at: expires,
            grace_until: None,
        },
    };
    svc.subscriptions.upsert(&sub).await?;
    Ok(sub)
}

/// The caller's subscription; `Ok(sub)` only when currently active.
pub async fn require_active(svc: &AppServices, user_id: Uuid) -> Result<Subscription, SubError> {
    let now = Utc::now();
    let sub = svc
        .subscriptions
        .find_by_user(user_id)
        .await?
        .ok_or(SubError::NoSubscription)?;
    if sub.is_active(now) {
        Ok(sub)
    } else {
        // Grace keeps read access but blocks restricted actions.
        Err(SubError::Expired)
    }
}

/// Enforces the per-subscription shop limit. Shop creation is free so users
/// can onboard first; usage inside the shop is what the gate blocks.
pub async fn ensure_can_create_shop(svc: &AppServices, user_id: Uuid) -> Result<(), SubError> {
    let owned = svc.shops.find_for_user(user_id).await?;
    let owned_count = owned.iter().filter(|s| s.owner_id == user_id).count();
    if owned_count >= MAX_SHOPS_PER_USER {
        return Err(SubError::QuotaShops);
    }
    Ok(())
}

/// Records storage usage for `key` (idempotent) within the shop quota.
pub async fn record_storage(
    svc: &AppServices,
    shop_id: Uuid,
    key: Uuid,
    bytes: i64,
) -> Result<i64, SubError> {
    use crate::domain::entities::subscription::MAX_STORAGE_BYTES;
    svc.storage
        .add_checked(shop_id, key, bytes, MAX_STORAGE_BYTES)
        .await
        .map_err(SubError::from)
}

/// `(key, bytes)` objects recorded against the shop quota.
pub async fn list_storage(svc: &AppServices, shop_id: Uuid) -> Result<Vec<(Uuid, i64)>, SubError> {
    svc.storage.list(shop_id).await.map_err(SubError::from)
}

/// Frees a storage object's bytes; returns the shop total after removal.
pub async fn remove_storage(svc: &AppServices, shop_id: Uuid, key: Uuid) -> Result<i64, SubError> {
    svc.storage
        .remove(shop_id, key)
        .await
        .map_err(SubError::from)
}

/// USDC minor units for `amount` (6 decimals).
#[must_use]
pub fn to_usdc_minor(amount: Decimal) -> u64 {
    (amount * Decimal::new(10i64.pow(USDC_DECIMALS), 0))
        .round()
        .to_u64_saturating()
}

trait DecimalExt {
    fn to_u64_saturating(&self) -> u64;
}

impl DecimalExt for Decimal {
    fn to_u64_saturating(&self) -> u64 {
        use rust_decimal::prelude::ToPrimitive;
        self.to_u64().unwrap_or(u64::MAX)
    }
}
