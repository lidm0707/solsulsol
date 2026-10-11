//! Canonical route paths for the POS API.

pub const HEALTH: &str = "/health";
pub const AUTH_NONCE: &str = "/auth/nonce";
pub const AUTH_VERIFY: &str = "/auth/verify";
pub const AUTH_LOGOUT: &str = "/auth/logout";
pub const AUTH_ME: &str = "/auth/me";
pub const PAYMENTS_ORDERS: &str = "/payments/orders";
pub const PAYMENTS_VERIFY: &str = "/payments/verify";
pub const SUBSCRIPTIONS_CURRENT: &str = "/subscriptions/current";
pub const SUBSCRIPTIONS_RENEW: &str = "/subscriptions/renew";

pub const SHOPS: &str = "/shops";
pub const SHOP_INVITES: &str = "/shops/{shop_id}/invites";
pub const SHOP_MEMBERS: &str = "/shops/{shop_id}/members";
pub const SHOP_MEMBER_ROLE: &str = "/shops/{shop_id}/members/{user_id}";
pub const SHOP_GOODS: &str = "/shops/{shop_id}/goods";
pub const SHOP_GOODS_ID: &str = "/shops/{shop_id}/goods/{goods_id}";
pub const SHOP_GOODS_SALES: &str = "/shops/{shop_id}/goods/sales";
pub const SHOP_STORAGE_OBJECTS: &str = "/shops/{shop_id}/storage/objects";
pub const SHOP_STORAGE_OBJECT: &str = "/shops/{shop_id}/storage/objects/{key}";
pub const SHOP_DESTINATIONS: &str = "/shops/{shop_id}/destinations";
pub const SHOP_CHECKOUT: &str = "/shops/{shop_id}/checkout";
pub const SHOP_SETTINGS: &str = "/shops/{shop_id}/settings";
pub const SHOP_BILLINGS: &str = "/shops/{shop_id}/billings";
pub const SHOP_BILLING_PDF: &str = "/shops/{shop_id}/billings/{billing_id}/pdf";

pub const INVITE_ACCEPT: &str = "/invites/{invite_id}/accept";
