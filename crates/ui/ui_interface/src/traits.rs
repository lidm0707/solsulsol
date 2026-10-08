//! Platform-generic traits. Each UI backend (wasm, native) implements these.
//! Intentionally `!Send`-friendly (plain `async fn` in trait): wasm is single-threaded.

use pos_api::schema::{
    AddDestinationRequest, AddGoodsRequest, CheckoutRequest, CreateShopRequest,
    DestinationResponse, GoodsResponse, InviteAcceptRequest, InviteRequest, InviteResponse,
    InvoiceResponse, LoginRequest, LoginResponse, RegisterRequest, Role, ShopMemberResponse,
    ShopResponse, UpdateMemberRoleRequest, UpdateShopSettingsRequest,
};
use uuid::Uuid;

pub trait ApiClient {
    async fn register(&self, req: &RegisterRequest) -> Result<Uuid, String>;
    async fn login(&self, req: &LoginRequest) -> Result<LoginResponse, String>;
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
    -> Result<InvoiceResponse, String>;
    async fn list_invoices(
        &self,
        token: &str,
        shop_id: Uuid,
    ) -> Result<Vec<InvoiceResponse>, String>;
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
