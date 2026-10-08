//! Worker infra: D1 repositories + pure-Rust adapters + composition root.

pub mod adapters;
pub mod d1_repos;

use std::sync::Arc;

use crate::infra::adapters::{Argon2Hasher, JwtIssuer, PromptPayAdapter, QrAdapter};
use crate::infra::d1_repos::{
    D1DestinationRepo, D1InviteRepo, D1InvoiceRepo, D1MemberRepo, D1ProductRepo, D1ShopRepo,
    D1UserRepo,
};
use pos_app::service::use_cases::AppServices;

pub const DB_BINDING: &str = "my_db";
pub const JWT_SECRET_ENV: &str = "POS_JWT_SECRET";
pub const DEFAULT_JWT_SECRET: &str = "dev-only-secret-change-me";

pub fn wire_services(env: &worker::Env) -> Result<AppServices, String> {
    let db: d1_repos::Db = Arc::new(env.d1(DB_BINDING).map_err(|e| e.to_string())?);
    let secret = env
        .var(JWT_SECRET_ENV)
        .map(|v| v.to_string())
        .unwrap_or_else(|_| DEFAULT_JWT_SECRET.to_owned());
    AppServices::builder()
        .users(Arc::new(D1UserRepo { db: db.clone() }))
        .shops(Arc::new(D1ShopRepo { db: db.clone() }))
        .members(Arc::new(D1MemberRepo { db: db.clone() }))
        .products(Arc::new(D1ProductRepo { db: db.clone() }))
        .destinations(Arc::new(D1DestinationRepo { db: db.clone() }))
        .invites(Arc::new(D1InviteRepo { db: db.clone() }))
        .invoices(Arc::new(D1InvoiceRepo { db: db.clone() }))
        .hasher(Arc::new(Argon2Hasher::new()))
        .tokens(Arc::new(JwtIssuer::new(secret)))
        .qr(Arc::new(QrAdapter))
        .promptpay(Arc::new(PromptPayAdapter))
        .build()
}
