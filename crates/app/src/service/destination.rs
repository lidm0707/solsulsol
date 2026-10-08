//! Payment destination management (bank/chain addresses per shop).

use crate::domain::entities::PaymentDestination;
use crate::domain::value_objects::{ChainAddress, SplToken, new_id};

use crate::service::shop::{ShopError, require_min_role};
use crate::service::use_cases::AppServices;
use crate::domain::value_objects::Role;

pub async fn add_destination(
    svc: &AppServices,
    actor_id: uuid::Uuid,
    shop_id: uuid::Uuid,
    address: ChainAddress,
    token: SplToken,
) -> Result<PaymentDestination, ShopError> {
    require_min_role(svc, actor_id, shop_id, Role::Admin).await?;
    let destination = PaymentDestination {
        id: new_id(),
        shop_id,
        address,
        token,
    };
    tracing::info!(shop_id = %shop_id, destination_id = %destination.id, "destination added");
    svc.destinations.insert(&destination).await?;
    Ok(destination)
}

pub async fn list_destinations(
    svc: &AppServices,
    actor_id: uuid::Uuid,
    shop_id: uuid::Uuid,
) -> Result<Vec<PaymentDestination>, ShopError> {
    require_min_role(svc, actor_id, shop_id, Role::Employee).await?;
    Ok(svc.destinations.list_by_shop(shop_id).await?)
}

pub async fn remove_destination(
    svc: &AppServices,
    actor_id: uuid::Uuid,
    shop_id: uuid::Uuid,
    destination_id: uuid::Uuid,
) -> Result<(), ShopError> {
    require_min_role(svc, actor_id, shop_id, Role::Admin).await?;
    let dest = svc
        .destinations
        .find_by_id(destination_id)
        .await?
        .ok_or(ShopError::NotFound)?;
    if dest.shop_id != shop_id {
        return Err(ShopError::NotFound);
    }
    svc.destinations.delete(destination_id).await?;
    Ok(())
}
