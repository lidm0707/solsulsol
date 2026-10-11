//! Goods (goods) management.

use crate::domain::entities::Goods;
use crate::domain::entities::subscription::MAX_GOODS_PER_SHOP;
use crate::domain::value_objects::{Money, PastelColor, new_id};

use crate::domain::value_objects::Role;
use crate::service::shop::{ShopError, require_min_role};
use crate::service::subscription::{SubError, require_active};
use crate::service::use_cases::AppServices;

pub struct NewGoods<'a> {
    pub name: &'a str,
    pub price: Money,
    pub cost: rust_decimal::Decimal,
    pub units: i64,
    pub barcode: Option<String>,
    pub icon: crate::domain::value_objects::GoodsIcon,
    pub color: PastelColor,
    pub goods_type: crate::domain::value_objects::GoodsType,
}

pub async fn add_goods(
    svc: &AppServices,
    actor_id: uuid::Uuid,
    shop_id: uuid::Uuid,
    goods: NewGoods<'_>,
) -> Result<Goods, ShopError> {
    require_min_role(svc, actor_id, shop_id, Role::Admin).await?;
    require_active(svc, actor_id)
        .await
        .map_err(ShopError::Subscription)?;
    let existing = svc.goods.find_by_shop(shop_id).await?;
    if existing.len() >= MAX_GOODS_PER_SHOP {
        return Err(ShopError::Subscription(SubError::QuotaGoods));
    }
    let goods = Goods {
        id: new_id(),
        shop_id,
        name: goods.name.to_owned(),
        price: goods.price,
        cost: goods.cost,
        units: goods.units,
        barcode: goods.barcode,
        icon: goods.icon,
        color: goods.color,
        goods_type: goods.goods_type,
    };
    tracing::info!(shop_id = %shop_id, goods_id = %goods.id, "goods added");
    svc.goods.insert(&goods).await?;
    Ok(goods)
}

pub async fn list_goods(
    svc: &AppServices,
    actor_id: uuid::Uuid,
    shop_id: uuid::Uuid,
) -> Result<Vec<Goods>, ShopError> {
    require_min_role(svc, actor_id, shop_id, Role::Employee).await?;
    Ok(svc.goods.find_by_shop(shop_id).await?)
}

/// Admin delete; blocked while any billing line references the goods so
/// billing history keeps pointing at a real row.
pub async fn delete_goods(
    svc: &AppServices,
    actor_id: uuid::Uuid,
    shop_id: uuid::Uuid,
    goods_id: uuid::Uuid,
) -> Result<(), ShopError> {
    require_min_role(svc, actor_id, shop_id, Role::Admin).await?;
    let goods = svc
        .goods
        .find_by_id(goods_id)
        .await?
        .ok_or(ShopError::NotFound)?;
    if goods.shop_id != shop_id {
        return Err(ShopError::NotFound);
    }
    if svc.billings.goods_has_lines(goods_id).await? {
        return Err(ShopError::GoodsBillingd);
    }
    svc.goods.delete(goods_id).await?;
    tracing::info!(shop_id = %shop_id, goods_id = %goods_id, "goods deleted");
    Ok(())
}

pub async fn goods_sales(
    svc: &AppServices,
    actor_id: uuid::Uuid,
    shop_id: uuid::Uuid,
) -> Result<Vec<(uuid::Uuid, u64)>, ShopError> {
    require_min_role(svc, actor_id, shop_id, Role::Employee).await?;
    Ok(svc.billings.sales_by_goods(shop_id).await?)
}
