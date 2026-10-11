//! Platform-generic traits. Each UI backend (wasm, native) implements these.
//! Intentionally `!Send`-friendly (plain `async fn` in trait): wasm is single-threaded.

use pos_api::schema::{
    AddDestinationRequest, AddGoodsRequest, AuthNonceRequest, AuthNonceResponse, AuthResponse,
    AuthVerifyRequest, CheckoutRequest, CreatePaymentOrderRequest, CreateShopRequest,
    DestinationResponse, GoodsResponse, GoodsSalesResponse, InviteAcceptRequest, InviteRequest,
    InviteResponse, BillingResponse, MeResponse, PaymentOrderResponse, Role, ShopMemberResponse,
    ShopResponse, StorageObjectResponse, SubscriptionResponse, UpdateMemberRoleRequest,
    UpdateShopSettingsRequest, VerifyPaymentRequest,
};
use uuid::Uuid;

pub trait ApiClient {
    async fn auth_nonce(&self, req: &AuthNonceRequest) -> Result<AuthNonceResponse, String>;
    async fn auth_verify(&self, req: &AuthVerifyRequest) -> Result<AuthResponse, String>;
    async fn auth_logout(&self, token: &str) -> Result<(), String>;
    async fn auth_me(&self, token: &str) -> Result<MeResponse, String>;
    async fn create_payment_order(
        &self,
        token: &str,
        req: &CreatePaymentOrderRequest,
    ) -> Result<PaymentOrderResponse, String>;
    async fn verify_payment(
        &self,
        token: &str,
        req: &VerifyPaymentRequest,
    ) -> Result<SubscriptionResponse, String>;
    async fn subscription_current(&self, token: &str) -> Result<SubscriptionResponse, String>;
    async fn create_shop(
        &self,
        token: &str,
        req: &CreateShopRequest,
    ) -> Result<ShopResponse, String>;
    async fn list_shops(&self, token: &str) -> Result<Vec<ShopResponse>, String>;
    async fn update_shop_settings(
        &self,
        token: &str,
        shop_id: Uuid,
        req: &UpdateShopSettingsRequest,
    ) -> Result<(), String>;
    async fn invite(&self, token: &str, req: &InviteRequest) -> Result<InviteResponse, String>;
    async fn accept_invite(
        &self,
        token: &str,
        invite_id: Uuid,
        req: &InviteAcceptRequest,
    ) -> Result<ShopMemberResponse, String>;
    async fn list_members(
        &self,
        token: &str,
        shop_id: Uuid,
    ) -> Result<Vec<ShopMemberResponse>, String>;
    async fn set_member_role(
        &self,
        token: &str,
        shop_id: Uuid,
        user_id: Uuid,
        req: &UpdateMemberRoleRequest,
    ) -> Result<(), String>;
    async fn add_goods(&self, token: &str, req: &AddGoodsRequest) -> Result<GoodsResponse, String>;
    async fn list_goods(&self, token: &str, shop_id: Uuid) -> Result<Vec<GoodsResponse>, String>;
    async fn list_goods_sales(
        &self,
        token: &str,
        shop_id: Uuid,
    ) -> Result<Vec<GoodsSalesResponse>, String>;
    async fn add_destination(
        &self,
        token: &str,
        req: &AddDestinationRequest,
    ) -> Result<DestinationResponse, String>;
    async fn list_destinations(
        &self,
        token: &str,
        shop_id: Uuid,
    ) -> Result<Vec<DestinationResponse>, String>;
    async fn checkout(&self, token: &str, req: &CheckoutRequest)
    -> Result<BillingResponse, String>;
    async fn list_billings(
        &self,
        token: &str,
        shop_id: Uuid,
    ) -> Result<Vec<BillingResponse>, String>;
    async fn delete_goods(&self, token: &str, shop_id: Uuid, goods_id: Uuid) -> Result<(), String>;
    async fn list_storage_objects(
        &self,
        token: &str,
        shop_id: Uuid,
    ) -> Result<Vec<StorageObjectResponse>, String>;
    async fn delete_storage_object(
        &self,
        token: &str,
        shop_id: Uuid,
        key: Uuid,
    ) -> Result<(), String>;
}

/// Which invite rows the UI shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteFilter {
    Pending,
    All,
}

impl InviteFilter {
    #[must_use]
    pub fn keeps(self, role: Role) -> bool {
        match self {
            Self::All => true,
            Self::Pending => role != Role::SuperAdmin,
        }
    }
}

pub trait TokenStore {
    async fn save(&self, token: &str);
    async fn load(&self) -> Option<String>;
    async fn clear(&self);
}
