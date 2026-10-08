//! Storage ports.

use crate::domain::entities::{
    Invite, InviteStatus, Invoice, PaymentDestination, Product, Shop, ShopMember, User, VatMode,
};
use crate::domain::value_objects::Role;
use async_trait::async_trait;
use rust_decimal::Decimal;
use uuid::Uuid;

#[async_trait]
pub trait UserRepo: Send + Sync {
    async fn insert(&self, user: &User) -> Result<(), String>;
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, String>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, String>;
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
    ) -> Result<(), String> {
        Err("shop settings not supported".into())
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
pub trait ProductRepo: Send + Sync {
    async fn insert(&self, product: &Product) -> Result<(), String>;
    async fn find_by_shop(&self, shop_id: Uuid) -> Result<Vec<Product>, String>;
    /// Removes one unit from stock; errors when the product is out of stock.
    async fn take_units(&self, product_id: Uuid) -> Result<(), String>;
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
pub trait InvoiceRepo: Send + Sync {
    async fn insert(&self, invoice: &Invoice) -> Result<(), String>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Invoice>, String>;
    async fn list_by_shop(&self, shop_id: Uuid) -> Result<Vec<Invoice>, String>;
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
