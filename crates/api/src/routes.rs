//! Canonical route paths for the POS API.

pub const HEALTH: &str = "/health";
pub const REGISTER: &str = "/auth/register";
pub const LOGIN: &str = "/auth/login";

pub const SHOPS: &str = "/shops";
pub const SHOP_INVITES: &str = "/shops/{shop_id}/invites";
pub const SHOP_MEMBERS: &str = "/shops/{shop_id}/members";
pub const SHOP_MEMBER_ROLE: &str = "/shops/{shop_id}/members/{user_id}";
pub const SHOP_GOODS: &str = "/shops/{shop_id}/goods";
pub const SHOP_DESTINATIONS: &str = "/shops/{shop_id}/destinations";
pub const SHOP_CHECKOUT: &str = "/shops/{shop_id}/checkout";
pub const SHOP_SETTINGS: &str = "/shops/{shop_id}/settings";
pub const SHOP_BILLINGS: &str = "/shops/{shop_id}/billings";
pub const SHOP_BILLING_PDF: &str = "/shops/{shop_id}/billings/{billing_id}/pdf";

pub const INVITE_ACCEPT: &str = "/invites/{invite_id}/accept";
