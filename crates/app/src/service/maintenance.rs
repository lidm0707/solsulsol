//! Lifecycle sweep: expiry → grace → soft delete → hard delete.
//! Idempotent and safe to re-run on a schedule or cron.

use crate::config::PaymentConfig;
use crate::domain::entities::subscription::{Subscription, SubscriptionStatus};
use crate::service::use_cases::AppServices;
use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Default, Clone, Copy)]
pub struct MaintenanceReport {
    pub marked_grace: u64,
    pub marked_lapsed: u64,
    pub soft_deleted_shops: u64,
    pub hard_deleted_shops: u64,
    pub hard_deleted_billings: u64,
}

/// Runs one sweep pass. Every stage is a conditional update, so concurrent
/// or repeated runs are safe.
pub async fn run_maintenance(
    svc: &AppServices,
    cfg: &PaymentConfig,
    now: DateTime<Utc>,
) -> Result<MaintenanceReport, String> {
    let mut report = MaintenanceReport::default();

    for sub in svc.subscriptions.list_to_mark_grace(now).await? {
        let grace_until = sub.expires_at + Duration::days(cfg.grace_days);
        svc.subscriptions
            .set_status(sub.id, SubscriptionStatus::Grace, Some(grace_until))
            .await?;
        report.marked_grace += 1;
        tracing::info!(user_id = %sub.user_id, sub_id = %sub.id, %grace_until, "subscription expired, grace started");
    }

    for sub in svc.subscriptions.list_to_mark_lapsed(now).await? {
        let deleted = svc.shops.soft_delete_for_owner(sub.user_id, now).await?;
        svc.subscriptions
            .set_status(sub.id, SubscriptionStatus::Lapsed, sub.grace_until)
            .await?;
        report.marked_lapsed += 1;
        report.soft_deleted_shops += deleted;
        tracing::warn!(user_id = %sub.user_id, sub_id = %sub.id, shops = deleted, "grace ended, shop data soft-deleted");
    }

    let purge_before = now - Duration::days(cfg.recovery_days.max(0));
    for sub in svc.subscriptions.list_to_delete(purge_before).await? {
        // Billings first — they reference shops without ON DELETE CASCADE.
        let billings = svc.billings.hard_delete_expired(purge_before).await?;
        let deleted = svc.shops.hard_delete_expired(purge_before).await?;
        svc.subscriptions
            .set_status(sub.id, SubscriptionStatus::Deleted, sub.grace_until)
            .await?;
        report.hard_deleted_shops += deleted;
        report.hard_deleted_billings += billings;
        tracing::warn!(user_id = %sub.user_id, sub_id = %sub.id, shops = deleted, billings, "recovery window passed, shop data permanently deleted");
    }

    Ok(report)
}

/// True when the subscription will enter grace within `warn_days` — used by
/// the UI/dashboard for renewal nudges.
#[must_use]
pub fn expiring_soon(sub: &Subscription, now: DateTime<Utc>, warn_days: i64) -> bool {
    sub.status == SubscriptionStatus::Active && sub.expires_at - now <= Duration::days(warn_days)
}
