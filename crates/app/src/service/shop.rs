//! Shop lifecycle: create, invites, member role management.

use crate::domain::entities::{Currency, Invite, InviteStatus, Shop, ShopMember, User, VatMode};
use crate::domain::services::{
    DomainRuleError, accept_invite as domain_accept, can_change_role, can_invite,
};
use crate::domain::value_objects::{Email, Role, new_id};

use crate::service::subscription::{SubError, ensure_can_create_shop};
use crate::service::use_cases::AppServices;

pub const INVITE_TTL_HOURS: i64 = 48;
pub const VAT_RATE_MIN: rust_decimal::Decimal = rust_decimal::Decimal::ZERO;
pub const VAT_RATE_MAX: rust_decimal::Decimal =
    rust_decimal::Decimal::from_parts(100, 0, 0, false, 0);

#[derive(Debug)]
pub enum ShopError {
    NotMember,
    Forbidden,
    InviteeUnknown,
    InviteNotAcceptable,
    NotFound,
    Subscription(SubError),
    Rule(DomainRuleError),
    Storage(String),
}

impl From<String> for ShopError {
    fn from(e: String) -> Self {
        ShopError::Storage(e)
    }
}

pub async fn create_shop(
    svc: &AppServices,
    owner_id: uuid::Uuid,
    name: &str,
) -> Result<Shop, ShopError> {
    let shop = Shop {
        id: new_id(),
        owner_id,
        name: name.to_owned(),
        vat_mode: VatMode::default(),
        vat_rate: rust_decimal::Decimal::ZERO,
        fx_thb_per_sol: rust_decimal::Decimal::ZERO,
        currency: Currency::default(),
        deleted_at: None,
    };
    tracing::info!(shop_id = %shop.id, "creating shop");
    ensure_can_create_shop(svc, owner_id)
        .await
        .map_err(ShopError::Subscription)?;
    svc.shops.insert(&shop).await?;
    let member = ShopMember {
        shop_id: shop.id,
        user_id: owner_id,
        role: Role::Owner,
    };
    svc.members.insert(&member).await?;
    Ok(shop)
}

/// Resolve the caller's role in a shop; `Err(NotMember)` when absent.
pub async fn member_role(
    svc: &AppServices,
    user_id: uuid::Uuid,
    shop_id: uuid::Uuid,
) -> Result<Role, ShopError> {
    svc.members
        .find_role(shop_id, user_id)
        .await?
        .ok_or(ShopError::NotMember)
}

/// Require at least `min` rank.
pub async fn require_min_role(
    svc: &AppServices,
    user_id: uuid::Uuid,
    shop_id: uuid::Uuid,
    min: Role,
) -> Result<Role, ShopError> {
    let role = member_role(svc, user_id, shop_id).await?;
    if role >= min {
        Ok(role)
    } else {
        Err(ShopError::Forbidden)
    }
}

pub async fn invite(
    svc: &AppServices,
    inviter_id: uuid::Uuid,
    shop_id: uuid::Uuid,
    invitee_email: &str,
    role: Role,
) -> Result<Invite, ShopError> {
    let actor = require_min_role(svc, inviter_id, shop_id, Role::Admin).await?;
    can_invite(actor, role).map_err(ShopError::Rule)?;
    let invitee = Email::parse(invitee_email).map_err(|_| ShopError::NotFound)?;
    // Invitee may not have an account yet; they register, then accept.
    let invite = Invite {
        id: new_id(),
        shop_id,
        email: invitee,
        role,
        status: InviteStatus::Pending,
        expires_at: chrono::Utc::now() + chrono::Duration::hours(INVITE_TTL_HOURS),
    };
    tracing::info!(shop_id = %shop_id, invite_id = %invite.id, ?role, "invite created");
    svc.invites.insert(&invite).await?;
    Ok(invite)
}

/// Accept a pending, unexpired invite addressed to `user`.
pub async fn accept_invite(
    svc: &AppServices,
    user: &User,
    invite_id: uuid::Uuid,
) -> Result<ShopMember, ShopError> {
    let invite = svc
        .invites
        .find_by_id(invite_id)
        .await?
        .ok_or(ShopError::NotFound)?;
    let member = domain_accept(&invite, user, chrono::Utc::now())
        .map_err(|_| ShopError::InviteNotAcceptable)?;
    svc.members.insert(&member).await?;
    svc.invites
        .set_status(invite.id, InviteStatus::Accepted)
        .await?;
    tracing::info!(shop_id = %member.shop_id, user_id = %member.user_id, "invite accepted");
    Ok(member)
}

/// Update shop sale settings (VAT mode + rate). Admins and above only.
pub async fn update_settings(
    svc: &AppServices,
    actor_id: uuid::Uuid,
    shop_id: uuid::Uuid,
    vat_mode: VatMode,
    vat_rate: rust_decimal::Decimal,
    fx_thb_per_sol: rust_decimal::Decimal,
    currency: Currency,
) -> Result<(), ShopError> {
    require_min_role(svc, actor_id, shop_id, Role::Admin).await?;
    if vat_rate < VAT_RATE_MIN || vat_rate > VAT_RATE_MAX {
        return Err(ShopError::Forbidden);
    }
    if fx_thb_per_sol < rust_decimal::Decimal::ZERO {
        return Err(ShopError::Forbidden);
    }
    svc.shops
        .update_settings(shop_id, vat_mode, vat_rate, fx_thb_per_sol, currency)
        .await?;
    tracing::info!(shop_id = %shop_id, ?vat_mode, %vat_rate, ?currency, "shop settings updated");
    Ok(())
}

pub async fn set_member_role(
    svc: &AppServices,
    actor_id: uuid::Uuid,
    shop_id: uuid::Uuid,
    target_id: uuid::Uuid,
    new_role: Role,
) -> Result<(), ShopError> {
    let actor = require_min_role(svc, actor_id, shop_id, Role::Admin).await?;
    let target = member_role(svc, target_id, shop_id).await?;
    can_change_role(actor, target, new_role).map_err(ShopError::Rule)?;
    svc.members
        .update_role(shop_id, target_id, new_role)
        .await?;
    tracing::info!(shop_id = %shop_id, user_id = %target_id, ?new_role, "member role updated");
    Ok(())
}

/// Members with emails joined in the app layer (repos never JOIN).
pub async fn list_members(
    svc: &AppServices,
    shop_id: uuid::Uuid,
) -> Result<Vec<(ShopMember, Option<String>)>, ShopError> {
    let members = svc.members.list_by_shop(shop_id).await?;
    let mut out = Vec::with_capacity(members.len());
    for m in members {
        let owner = svc.users.find_by_id(m.user_id).await?;
        let label = owner.map(|u| u.label().to_owned()).unwrap_or_default();
        out.push((m, (!label.is_empty()).then_some(label)));
    }
    Ok(out)
}
