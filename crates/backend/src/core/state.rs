//! Composition root helpers: wiring adapters into ports.
//! Only this module may depend on every layer.

use crate::app::service::use_cases::AppServices;
use crate::infra::database_adapter::repos;
use crate::infra::encryption_adapter::hash::Argon2Hasher;
use crate::infra::encryption_adapter::jwt::JwtIssuer;
use crate::infra::qr_adapter::adapter::QrAdapter;
use crate::infra::promptpay_adapter::adapter::PromptPayAdapter;

pub const JWT_SECRET_ENV: &str = "POS_JWT_SECRET";
pub const DATABASE_URL_ENV: &str = "POS_DATABASE_URL";
pub const DEFAULT_JWT_SECRET: &str = "dev-only-secret-change-me";

pub async fn wire_services() -> Result<AppServices, String> {
    let secret = std::env::var(JWT_SECRET_ENV).unwrap_or_else(|_| {
        tracing::warn!("{} not set, using dev secret", JWT_SECRET_ENV);
        DEFAULT_JWT_SECRET.to_owned()
    });
    let url = std::env::var(DATABASE_URL_ENV)
        .map_err(|_| format!("{} must be set (postgres)", DATABASE_URL_ENV))?;
    let builder = AppServices::builder()
        .hasher(std::sync::Arc::new(Argon2Hasher::new()))
        .tokens(std::sync::Arc::new(JwtIssuer::new(secret)))
        .qr(std::sync::Arc::new(QrAdapter))
        .promptpay(std::sync::Arc::new(PromptPayAdapter));
    wire_postgres(builder, &url).await
}

async fn wire_postgres(builder: AppServicesBuilder, url: &str) -> Result<AppServices, String> {
    let pool = repos::connect(url).await.map_err(|e| e.to_string())?;
    run_migrations(&pool).await?;
    builder
        .users(std::sync::Arc::new(repos::PgUserRepo {
            pool: pool.clone(),
        }))
        .shops(std::sync::Arc::new(repos::PgShopRepo {
            pool: pool.clone(),
        }))
        .members(std::sync::Arc::new(repos::PgMemberRepo {
            pool: pool.clone(),
        }))
        .products(std::sync::Arc::new(repos::PgProductRepo {
            pool: pool.clone(),
        }))
        .destinations(std::sync::Arc::new(repos::PgDestinationRepo {
            pool: pool.clone(),
        }))
        .invites(std::sync::Arc::new(repos::PgInviteRepo {
            pool: pool.clone(),
        }))
        .invoices(std::sync::Arc::new(repos::PgInvoiceRepo {
            pool: pool.clone(),
        }))
        .tx(std::sync::Arc::new(repos::PgTxManager { pool }))
        .build()
}

use crate::app::service::use_cases::AppServicesBuilder;

async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), String> {
    sqlx::migrate!("../../docker/migrations")
        .run(pool)
        .await
        .map_err(|e| e.to_string())
}
