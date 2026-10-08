use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    SuperAdmin,
    Owner,
    Admin,
    Employee,
}

impl Role {
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::SuperAdmin => 3,
            Self::Owner => 2,
            Self::Admin => 1,
            Self::Employee => 0,
        }
    }

    #[must_use]
    pub const fn can_manage(self, other: Self) -> bool {
        self.rank() > other.rank()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Chain {
    Solana,
    Eth,
    Btc,
    Binance,
    Bank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplToken {
    #[default]
    Sol,
    Usdc,
    Usdt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub user_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateShopRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopResponse {
    pub shop_id: Uuid,
    pub name: String,
    #[serde(default)]
    pub vat_mode: VatMode,
    #[serde(default)]
    pub vat_rate: rust_decimal::Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopMemberResponse {
    pub user_id: Uuid,
    pub email: String,
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMemberRoleRequest {
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteRequest {
    pub shop_id: Uuid,
    pub email: String,
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteResponse {
    pub invite_id: Uuid,
    pub shop_id: Uuid,
    pub email: String,
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteAcceptRequest {
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddGoodsRequest {
    pub shop_id: Uuid,
    pub name: String,
    pub price: rust_decimal::Decimal,
    #[serde(default)]
    pub cost: rust_decimal::Decimal,
    #[serde(default)]
    pub units: i64,
    #[serde(default)]
    pub barcode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoodsResponse {
    pub goods_id: Uuid,
    pub shop_id: Uuid,
    pub name: String,
    pub price: rust_decimal::Decimal,
    #[serde(default)]
    pub cost: rust_decimal::Decimal,
    #[serde(default)]
    pub units: i64,
    #[serde(default)]
    pub barcode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddDestinationRequest {
    pub shop_id: Uuid,
    pub chain: Chain,
    pub address: String,
    #[serde(default)]
    pub token: SplToken,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DestinationResponse {
    pub destination_id: Uuid,
    pub shop_id: Uuid,
    pub chain: Chain,
    pub address: String,
    #[serde(default)]
    pub token: SplToken,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutRequest {
    pub shop_id: Uuid,
    pub goods_ids: Vec<Uuid>,
    pub destination_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvoiceLineResponse {
    pub name: String,
    pub price: rust_decimal::Decimal,
    pub qty: u32,
    pub cost: rust_decimal::Decimal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvoiceResponse {
    pub invoice_id: Uuid,
    pub shop_id: Uuid,
    pub total: rust_decimal::Decimal,
    #[serde(default)]
    pub subtotal: rust_decimal::Decimal,
    #[serde(default)]
    pub vat_mode: VatMode,
    #[serde(default)]
    pub vat_rate: rust_decimal::Decimal,
    #[serde(default)]
    pub vat_amount: rust_decimal::Decimal,
    pub cost: rust_decimal::Decimal,
    pub lines: Vec<InvoiceLineResponse>,
    pub destination: DestinationResponse,
    pub qr_payload: String,
    pub pdf_url: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    Pending,
    Paid,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VatMode {
    #[default]
    None,
    Exclusive,
    Inclusive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateShopSettingsRequest {
    pub vat_mode: VatMode,
    pub vat_rate: rust_decimal::Decimal,
}
