//! Goods (products) management.

use crate::domain::entities::Product;
use crate::domain::value_objects::{Money, new_id};

use crate::app::service::shop::{ShopError, require_min_role};
use crate::app::service::use_cases::AppServices;
use crate::domain::value_objects::Role;

pub struct NewGoods<'a> {
    pub name: &'a str,
    pub price: Money,
    pub cost: rust_decimal::Decimal,
    pub units: i64,
    pub barcode: Option<String>,
}

pub async fn add_goods(
    svc: &AppServices,
    actor_id: uuid::Uuid,
    shop_id: uuid::Uuid,
    goods: NewGoods<'_>,
) -> Result<Product, ShopError> {
    require_min_role(svc, actor_id, shop_id, Role::Admin).await?;
    let product = Product {
        id: new_id(),
        shop_id,
        name: goods.name.to_owned(),
        price: goods.price,
        cost: goods.cost,
        units: goods.units,
        barcode: goods.barcode,
    };
    tracing::info!(shop_id = %shop_id, product_id = %product.id, "goods added");
    svc.products.insert(&product).await?;
    Ok(product)
}

pub async fn list_goods(
    svc: &AppServices,
    actor_id: uuid::Uuid,
    shop_id: uuid::Uuid,
) -> Result<Vec<Product>, ShopError> {
    require_min_role(svc, actor_id, shop_id, Role::Employee).await?;
    Ok(svc.products.find_by_shop(shop_id).await?)
}
