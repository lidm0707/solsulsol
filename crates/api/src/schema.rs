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
pub struct AuthNonceRequest {
    pub wallet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthNonceResponse {
    pub nonce: String,
    pub message: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthVerifyRequest {
    pub wallet: String,
    pub nonce: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: Uuid,
    pub wallet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeResponse {
    pub user_id: Uuid,
    pub wallet: Option<String>,
    pub email: Option<String>,
    pub role: Role,
    #[serde(default)]
    pub subscription: Option<SubscriptionResponse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatusApi {
    Active,
    Grace,
    Lapsed,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionResponse {
    pub status: SubscriptionStatusApi,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub grace_until: Option<chrono::DateTime<chrono::Utc>>,
    pub price_usdc: rust_decimal::Decimal,
    pub shops_used: usize,
    pub shops_quota: usize,
    pub goods_used: usize,
    pub goods_quota: usize,
    pub storage_bytes: i64,
    pub storage_quota_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageObjectResponse {
    pub key: Uuid,
    pub bytes: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingPeriodApi {
    Monthly,
    Yearly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePaymentOrderRequest {
    #[serde(default)]
    pub renew: bool,
    #[serde(default)]
    pub period: Option<BillingPeriodApi>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentOrderStatusApi {
    Pending,
    Paid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentOrderResponse {
    pub order_id: Uuid,
    pub amount_usdc: rust_decimal::Decimal,
    pub mint: String,
    pub recipient: String,
    pub status: PaymentOrderStatusApi,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyPaymentRequest {
    pub order_id: Uuid,
    pub signature: String,
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
    #[serde(default)]
    pub fx_thb_per_usd: rust_decimal::Decimal,
    #[serde(default)]
    pub currency: Currency,
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
pub struct InviteAcceptRequest {}

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
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub goods_type: String,
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
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub goods_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoodsSalesResponse {
    pub goods_id: Uuid,
    pub sold: u64,
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
pub struct BillingLineResponse {
    pub name: String,
    pub price: rust_decimal::Decimal,
    pub qty: u32,
    pub cost: rust_decimal::Decimal,
}

pub const DECIMAL_ZERO: rust_decimal::Decimal = rust_decimal::Decimal::ZERO;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BillingResponse {
    pub billing_id: Uuid,
    pub shop_id: Uuid,
    pub total: rust_decimal::Decimal,
    /// Amount encoded in the QR payload (THB for PromptPay, USD-token amount
    /// for Solana — see `billing::qr_amount`; may differ from `total`).
    #[serde(default)]
    pub qr_amount: rust_decimal::Decimal,
    #[serde(default)]
    pub subtotal: rust_decimal::Decimal,
    #[serde(default)]
    pub vat_mode: VatMode,
    #[serde(default)]
    pub vat_rate: rust_decimal::Decimal,
    #[serde(default)]
    pub vat_amount: rust_decimal::Decimal,
    pub cost: rust_decimal::Decimal,
    pub lines: Vec<BillingLineResponse>,
    pub destination: DestinationResponse,
    pub qr_payload: String,
    pub pdf_url: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingStatus {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Currency {
    Thb,
    #[default]
    Usdt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateShopSettingsRequest {
    pub vat_mode: VatMode,
    pub vat_rate: rust_decimal::Decimal,
    #[serde(default)]
    pub fx_thb_per_usd: rust_decimal::Decimal,
    #[serde(default)]
    pub currency: Currency,
}
