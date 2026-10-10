//! Domain services: pure business rules, no I/O here.

use crate::domain::entities::{Invite, Invoice, Order, ShopMember, User};
use crate::domain::value_objects::Role;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainRuleError {
    #[error("actor role is not allowed to perform this action")]
    Forbidden,
    #[error("invite is not acceptable")]
    InviteNotAcceptable,
    #[error("order is empty or goods from another shop")]
    InvalidOrder,
}

/// Who may invite a given role into a shop: actor outranks invited role.
pub fn can_invite(actor: Role, invited: Role) -> Result<(), DomainRuleError> {
    if actor.can_manage(invited) {
        Ok(())
    } else {
        Err(DomainRuleError::Forbidden)
    }
}

/// Who may change a member's role: actor outranks both target and new role,
/// and nobody may demote an owner except a super admin (owner of shop guard is in app layer).
pub fn can_change_role(actor: Role, target: Role, new_role: Role) -> Result<(), DomainRuleError> {
    if actor.can_manage(target) && actor.can_manage(new_role) {
        Ok(())
    } else {
        Err(DomainRuleError::Forbidden)
    }
}

/// A member may act on the shop only if they belong to it (employee included).
pub fn is_shop_member(shop_id: uuid::Uuid, member: &ShopMember) -> bool {
    member.shop_id == shop_id
}

/// Accept an invite if pending and not expired. Email must match when the
/// user has one linked; wallet-only users accept by invite id (capability).
pub fn accept_invite(
    invite: &Invite,
    user: &User,
    now: DateTime<Utc>,
) -> Result<ShopMember, DomainRuleError> {
    if let Some(email) = &user.email
        && email.as_str() != invite.email.as_str()
    {
        return Err(DomainRuleError::Forbidden);
    }
    if !invite.is_acceptable(now) {
        return Err(DomainRuleError::InviteNotAcceptable);
    }
    Ok(ShopMember {
        shop_id: invite.shop_id,
        user_id: user.id,
        role: invite.role,
    })
}

/// Validate an order: non-empty, all lines priced positively (Money guarantees > 0).
pub fn validate_order(order: &Order) -> Result<Decimal, DomainRuleError> {
    if order.lines.is_empty() {
        return Err(DomainRuleError::InvalidOrder);
    }
    Ok(order.total())
}

/// A pending invoice may transition to paid or cancelled only.
pub fn invoice_can_transition(
    invoice: &Invoice,
    target: crate::domain::entities::InvoiceStatus,
) -> bool {
    use crate::domain::entities::InvoiceStatus::*;
    matches!(
        (invoice.status, target),
        (Pending, Paid) | (Pending, Cancelled)
    )
}
