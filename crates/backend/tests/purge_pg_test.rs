//! Purge semantics against real Postgres: when the recovery window passes,
//! the shop and ALL its data (goods, members, destinations, billings + lines,
//! storage rows) are deleted, and the billings-first order keeps the FKs happy.
//! Runs only when `PURGE_PG_URL` is set (e.g. the e2e compose database).

#![allow(clippy::todo, clippy::unimplemented)]

use pos_backend::app::config::PaymentConfig;
use pos_backend::app::service::maintenance::run_maintenance;
use pos_backend::app::service::use_cases::AppServices;
use pos_backend::domain::entities::subscription::{Subscription, SubscriptionStatus};
use pos_backend::domain::entities::{
    Billing, BillingLine, BillingStatus, Goods, PaymentDestination, ShopMember, User,
};
use pos_backend::domain::value_objects::{Chain, ChainAddress, Money, Role, SplToken, Wallet};
use pos_backend::infra::database_adapter::repos as pg;
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

fn pg_url() -> Option<String> {
    std::env::var("PURGE_PG_URL").ok()
}

async fn wire(url: &str) -> (AppServices, sqlx::PgPool) {
    let pool = pg::connect(url).await.expect("connect");
    sqlx::migrate!("../../docker/migrations")
        .run(&pool)
        .await
        .expect("migrations");
    let svc = AppServices::builder(PaymentConfig::devnet(
        "6CzkLVRqUSmw8pR3S8b66mc7QNwT6Ftyw9csXvKr6eef",
    ))
    .users(Arc::new(pg::PgUserRepo { pool: pool.clone() }))
    .shops(Arc::new(pg::PgShopRepo { pool: pool.clone() }))
    .members(Arc::new(pg::PgMemberRepo { pool: pool.clone() }))
    .goods(Arc::new(pg::PgGoodsRepo { pool: pool.clone() }))
    .destinations(Arc::new(pg::PgDestinationRepo { pool: pool.clone() }))
    .invites(Arc::new(pg::PgInviteRepo { pool: pool.clone() }))
    .billings(Arc::new(pg::PgBillingRepo { pool: pool.clone() }))
    .nonces(Arc::new(pg::PgNonceRepo { pool: pool.clone() }))
    .subscriptions(Arc::new(pg::PgSubscriptionRepo { pool: pool.clone() }))
    .orders(Arc::new(pg::PgPaymentOrderRepo { pool: pool.clone() }))
    .storage(Arc::new(pg::PgStorageRepo { pool: pool.clone() }))
    .tokens(Arc::new(
        pos_backend::infra::encryption_adapter::jwt::JwtIssuer::new("purge-test-secret".to_owned()),
    ))
    .wallet_verifier(Arc::new(
        pos_backend::infra::solana_adapter::signature::Ed25519Verifier,
    ))
    .payment_verifier(Arc::new(
        pos_backend::infra::solana_adapter::rpc::RpcPaymentVerifier::new("http://127.0.0.1:8899"),
    ))
    .qr(Arc::new(pos_backend::infra::qr_adapter::adapter::QrAdapter))
    .promptpay(Arc::new(
        pos_backend::infra::promptpay_adapter::adapter::PromptPayAdapter,
    ))
    .build()
    .expect("services wired");
    (svc, pool)
}

fn user(id: Uuid) -> User {
    // base58-only wallet string: uuid simple hex has 0/8/9/b.. — keep valid
    // chars and pad to the minimum Solana length.
    let raw = format!(
        "{}{}{}{}",
        Uuid::now_v7().simple(),
        Uuid::now_v7().simple(),
        Uuid::now_v7().simple(),
        Uuid::now_v7().simple()
    );
    let wallet: String = raw
        .chars()
        .filter(|c| !matches!(c, '0' | 'O' | 'I' | 'l'))
        .take(40)
        .collect();
    User {
        id,
        email: None,
        wallet: Some(Wallet::parse(&wallet).expect("base58 wallet")),
        role: Role::Employee,
        created_at: chrono::Utc::now(),
    }
}

#[tokio::test]
async fn purge_removes_every_shop_data_including_billings() {
    let Some(url) = pg_url() else {
        eprintln!("PURGE_PG_URL not set — skipping");
        return;
    };
    let (svc, pool) = wire(&url).await;
    let uid = Uuid::now_v7();
    let other = Uuid::now_v7();
    svc.users.insert(&user(uid)).await.unwrap();
    svc.users.insert(&user(other)).await.unwrap();
    for (id, days) in [(uid, 30_i64), (other, 4000)] {
        let now = chrono::Utc::now();
        svc.subscriptions
            .upsert(&Subscription {
                id: Uuid::now_v7(),
                user_id: id,
                status: SubscriptionStatus::Active,
                started_at: now,
                expires_at: now + chrono::Duration::days(days),
                grace_until: None,
            })
            .await
            .unwrap();
    }

    let shop = pos_backend::app::service::shop::create_shop(&svc, uid, "doomed")
        .await
        .unwrap();
    let other_shop = pos_backend::app::service::shop::create_shop(&svc, other, "untouched")
        .await
        .unwrap();
    svc.members
        .insert(&ShopMember {
            shop_id: shop.id,
            user_id: uid,
            role: Role::Owner,
        })
        .await
        .unwrap_err(); // owner row already inserted by create_shop
    let goods = Goods {
        id: Uuid::now_v7(),
        shop_id: shop.id,
        name: "latte".into(),
        price: Money::one(),
        cost: Decimal::ZERO,
        units: 5,
        barcode: None,
        icon: Default::default(),
        color: Default::default(),
        goods_type: Default::default(),
    };
    svc.goods.insert(&goods).await.unwrap();
    let destination = PaymentDestination {
        id: Uuid::now_v7(),
        shop_id: shop.id,
        address: ChainAddress::parse(
            Chain::Solana,
            "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR",
        )
        .expect("valid solana address"),
        token: SplToken::Usdc,
    };
    svc.destinations.insert(&destination).await.unwrap();
    svc.storage
        .add_checked(shop.id, Uuid::now_v7(), 128, 100_000)
        .await
        .unwrap();
    let billing = Billing {
        id: Uuid::now_v7(),
        shop_id: shop.id,
        status: BillingStatus::Paid,
        lines: vec![BillingLine {
            goods_id: goods.id,
            name: "latte".into(),
            price: Money::one(),
            cost: Decimal::ZERO,
            qty: 1,
        }],
        destination: destination.clone(),
        vat_mode: Default::default(),
        vat_rate: Decimal::ZERO,
    };
    svc.billings.insert(&billing).await.unwrap();

    // Expire → grace → lapsed (soft delete) → past recovery: purge.
    let now = chrono::Utc::now();
    let sub = svc.subscriptions.find_by_user(uid).await.unwrap().unwrap();
    svc.subscriptions
        .upsert(&Subscription {
            expires_at: now - chrono::Duration::days(1),
            grace_until: None,
            ..sub.clone()
        })
        .await
        .unwrap();
    let report = run_maintenance(&svc, &svc.payment_config, now)
        .await
        .unwrap();
    assert_eq!(report.marked_grace, 1);
    let now = chrono::Utc::now() + chrono::Duration::days(31);
    let report = run_maintenance(&svc, &svc.payment_config, now)
        .await
        .unwrap();
    assert_eq!(report.marked_lapsed, 1);
    let now = now + chrono::Duration::days(8);
    let report = run_maintenance(&svc, &svc.payment_config, now)
        .await
        .unwrap();
    assert_eq!(report.hard_deleted_shops, 1);
    assert_eq!(report.hard_deleted_billings, 1);

    // Everything of the doomed shop is gone.
    assert!(svc.shops.find_by_id(shop.id).await.unwrap().is_none());
    assert!(svc.goods.find_by_id(goods.id).await.unwrap().is_none());
    assert!(svc.billings.find_by_id(billing.id).await.unwrap().is_none());
    let lines: i64 = sqlx::query_scalar("SELECT count(*) FROM billing_lines WHERE billing_id = $1")
        .bind(billing.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(lines, 0, "billing lines purged");
    let members: i64 = sqlx::query_scalar("SELECT count(*) FROM shop_members WHERE shop_id = $1")
        .bind(shop.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(members, 0);
    let storage: i64 = sqlx::query_scalar("SELECT count(*) FROM storage_files WHERE shop_id = $1")
        .bind(shop.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(storage, 0);
    let destinations: i64 =
        sqlx::query_scalar("SELECT count(*) FROM destinations WHERE shop_id = $1")
            .bind(shop.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(destinations, 0);

    // The other user's shop is untouched.
    assert!(
        svc.shops
            .find_by_id(other_shop.id)
            .await
            .unwrap()
            .is_some_and(|s| s.deleted_at.is_none())
    );

    // Cleanup so the shared test DB stays reusable.
    for stmt in [
        "DELETE FROM subscriptions WHERE user_id = ANY($1)",
        "DELETE FROM shops WHERE owner_id = ANY($1)",
        "DELETE FROM users WHERE id = ANY($1)",
    ] {
        sqlx::query(stmt)
            .bind(vec![uid, other])
            .execute(&pool)
            .await
            .unwrap();
    }
}
