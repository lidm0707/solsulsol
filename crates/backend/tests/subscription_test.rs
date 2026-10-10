//! Wallet auth + USDC subscription acceptance tests (in-memory fakes).
#![allow(clippy::todo, clippy::unimplemented)]

mod common;

use common::{
    FakeChain, GOOD_SIG, GoodsRepo, NonceRepo, PaymentOrderRepo, RECIPIENT, ShopMemberRepo,
    ShopRepo, SubscriptionRepo, TEST_WALLET, UserRepo, grant_active, services, test_config,
    wallet_user,
};
use pos_backend::app::service::auth;
use pos_backend::app::service::maintenance::run_maintenance;
use pos_backend::app::service::shop::{self, ShopError};
use pos_backend::app::service::subscription::{
    self, SubError, add_month, record_storage, renewal_base, require_active,
};
use pos_backend::domain::entities::subscription::{
    MAX_GOODS_PER_SHOP, MAX_SHOPS_PER_USER, PaymentOrderStatus, SubscriptionStatus,
};
use pos_backend::domain::value_objects::Role;
use rust_decimal::Decimal;
use uuid::Uuid;

fn bad_config() -> pos_backend::app::config::PaymentConfig {
    test_config(RECIPIENT)
}

#[tokio::test]
async fn wallet_auth_creates_user_and_issues_token() {
    let (svc, w) = services(bad_config());
    let (nonce, _msg, _exp) = auth::nonce(&svc, TEST_WALLET).await.expect("nonce ok");
    let tokens = svc.tokens.clone();
    let verifier = svc.wallet_verifier.clone();
    let (user_id, token) = auth::verify(
        &svc,
        verifier.as_ref(),
        tokens.as_ref(),
        TEST_WALLET,
        &nonce,
        GOOD_SIG,
    )
    .await
    .expect("verify ok");
    assert_eq!(token, format!("token-{user_id}"));
    assert!(w.users.find_by_wallet(TEST_WALLET).await.unwrap().is_some());
}

#[tokio::test]
async fn wallet_auth_rejects_bad_signature() {
    let (svc, _w) = services(bad_config());
    let (nonce, _msg, _exp) = auth::nonce(&svc, TEST_WALLET).await.unwrap();
    let svc2 = svc.clone();
    let verifier = std::sync::Arc::new(common::StubVerifier { accept: false });
    let tokens = svc.tokens.clone();
    let err = auth::verify(
        &svc2,
        verifier.as_ref(),
        tokens.as_ref(),
        TEST_WALLET,
        &nonce,
        GOOD_SIG,
    )
    .await
    .expect_err("bad signature rejected");
    assert!(matches!(err, auth::AuthError::Invalid(_)));
}

#[tokio::test]
async fn nonce_is_single_use() {
    let (svc, _w) = services(bad_config());
    let (nonce, _msg, _exp) = auth::nonce(&svc, TEST_WALLET).await.unwrap();
    let verifier = svc.wallet_verifier.clone();
    let tokens = svc.tokens.clone();
    auth::verify(
        &svc,
        verifier.as_ref(),
        tokens.as_ref(),
        TEST_WALLET,
        &nonce,
        GOOD_SIG,
    )
    .await
    .expect("first use ok");
    let err = auth::verify(
        &svc,
        verifier.as_ref(),
        tokens.as_ref(),
        TEST_WALLET,
        &nonce,
        GOOD_SIG,
    )
    .await
    .expect_err("replay rejected");
    assert!(matches!(err, auth::AuthError::Invalid(_)));
}

#[tokio::test]
async fn expired_nonce_rejected() {
    let (svc, w) = services(bad_config());
    let (nonce, _msg, _exp) = auth::nonce(&svc, TEST_WALLET).await.unwrap();
    // Simulate expiry by consuming far in the future.
    let consumed = w
        .nonces
        .consume(
            TEST_WALLET,
            &nonce,
            chrono::Utc::now() + chrono::Duration::days(1),
        )
        .await
        .unwrap();
    assert!(!consumed, "expired nonce must not be consumable");
}

#[tokio::test]
async fn payment_wrong_amount_rejected() {
    let (svc, w) = services(bad_config());
    let uid = Uuid::now_v7();
    w.users.insert(&wallet_user(uid)).await.unwrap();
    let order = subscription::create_order(&svc, uid).await.unwrap();
    w.verifier.set(FakeChain::WrongAmount);
    let err = subscription::verify_order(&svc, w.verifier.as_ref(), uid, order.id, "sig")
        .await
        .expect_err("wrong amount rejected");
    assert!(matches!(err, SubError::Verify(_)));
    let stored = w.orders.find_by_id(order.id).await.unwrap().unwrap();
    assert_eq!(stored.status, PaymentOrderStatus::Pending);
}

#[tokio::test]
async fn payment_wrong_recipient_or_mint_or_failed_tx_rejected() {
    for outcome in [
        FakeChain::WrongRecipient,
        FakeChain::WrongMint,
        FakeChain::Failed,
        FakeChain::NotFound,
    ] {
        let (svc, w) = services(bad_config());
        let uid = Uuid::now_v7();
        let order = subscription::create_order(&svc, uid).await.unwrap();
        w.verifier.set(outcome);
        let err = subscription::verify_order(&svc, w.verifier.as_ref(), uid, order.id, "sig")
            .await
            .expect_err("must be rejected");
        assert!(matches!(err, SubError::Verify(_)));
    }
}

#[tokio::test]
async fn duplicate_signature_cannot_double_credit() {
    let (svc, w) = services(bad_config());
    let uid = Uuid::now_v7();
    let o1 = subscription::create_order(&svc, uid).await.unwrap();
    let o2 = subscription::create_order(&svc, uid).await.unwrap();
    subscription::verify_order(&svc, w.verifier.as_ref(), uid, o1.id, "same-sig")
        .await
        .expect("first verify ok");
    // Same tx signature reused on another order: atomic guard refuses.
    let marked = w
        .orders
        .mark_paid_if_pending(o2.id, "same-sig", chrono::Utc::now())
        .await
        .unwrap();
    assert!(
        !marked,
        "duplicate tx signature must not mark a second order paid"
    );
}

#[tokio::test]
async fn order_owned_by_other_user_is_not_verifiable() {
    let (svc, w) = services(bad_config());
    let owner = Uuid::now_v7();
    let attacker = Uuid::now_v7();
    w.users.insert(&wallet_user(owner)).await.unwrap();
    let order = subscription::create_order(&svc, owner).await.unwrap();
    let err = subscription::verify_order(&svc, w.verifier.as_ref(), attacker, order.id, "sig")
        .await
        .expect_err("cross-user verify rejected");
    assert!(matches!(err, SubError::OrderNotFound));
}

#[tokio::test]
async fn shop_creation_requires_subscription() {
    let (svc, w) = services(bad_config());
    let uid = Uuid::now_v7();
    w.users.insert(&wallet_user(uid)).await.unwrap();
    assert!(matches!(
        shop::create_shop(&svc, uid, "nope").await,
        Err(ShopError::Subscription(SubError::NoSubscription))
    ));
    grant_active(&svc, uid).await;
    shop::create_shop(&svc, uid, "ok")
        .await
        .expect("with sub ok");
}

#[tokio::test]
async fn shop_quota_is_one_per_user() {
    let (svc, _w) = services(bad_config());
    let uid = Uuid::now_v7();
    grant_active(&svc, uid).await;
    shop::create_shop(&svc, uid, "one").await.unwrap();
    assert_eq!(MAX_SHOPS_PER_USER, 1);
    assert!(matches!(
        shop::create_shop(&svc, uid, "two").await,
        Err(ShopError::Subscription(SubError::QuotaShops))
    ));
}

#[tokio::test]
async fn goods_quota_enforced() {
    let (svc, w) = services(bad_config());
    let uid = Uuid::now_v7();
    grant_active(&svc, uid).await;
    w.members
        .insert(&pos_backend::domain::entities::ShopMember {
            shop_id: SHOP_ID,
            user_id: uid,
            role: Role::Admin,
        })
        .await
        .unwrap();
    // Simulate the limit being reached already.
    for _ in 0..MAX_GOODS_PER_SHOP {
        w.goods
            .insert(&pos_backend::domain::entities::Goods {
                id: Uuid::now_v7(),
                shop_id: SHOP_ID,
                name: "p".into(),
                price: pos_backend::domain::value_objects::Money::one(),
                cost: Decimal::ZERO,
                units: 0,
                barcode: None,
                icon: Default::default(),
                color: Default::default(),
            goods_type: Default::default(),
            })
            .await
            .unwrap();
    }
    let money = pos_backend::domain::value_objects::Money::one();
    assert!(matches!(
        pos_backend::app::service::goods::add_goods(
            &svc,
            uid,
            SHOP_ID,
            pos_backend::app::service::goods::NewGoods {
                name: "overflow",
                price: money,
                cost: Decimal::ZERO,
                units: 0,
                barcode: None,
                icon: Default::default(),
                color: Default::default(),
            goods_type: Default::default(),
            },
        )
        .await,
        Err(ShopError::Subscription(SubError::QuotaGoods))
    ));
}

const SHOP_ID: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000002");

#[tokio::test]
async fn storage_quota_enforced_and_released() {
    let (svc, w) = services(bad_config());
    let uid = Uuid::now_v7();
    grant_active(&svc, uid).await;
    let shop = shop::create_shop(&svc, uid, "s").await.unwrap();
    let big = common::QUOTA;
    let ok = record_storage(&svc, shop.id, Uuid::now_v7(), big)
        .await
        .unwrap();
    assert_eq!(ok, big);
    let err = record_storage(&svc, shop.id, Uuid::now_v7(), 1)
        .await
        .expect_err("over quota rejected");
    assert!(matches!(err, SubError::QuotaStorage));
    // Same key re-record is idempotent (does not double count).
    let existing_key = {
        let rows = w.storage.rows.lock().unwrap();
        rows.keys().next().expect("one entry recorded").1
    };
    let again = record_storage(&svc, shop.id, existing_key, big)
        .await
        .unwrap();
    assert_eq!(again, big);
}

#[tokio::test]
async fn expired_subscription_blocks_actions_but_grace_keeps_data() {
    let (svc, w) = services(bad_config());
    let uid = Uuid::now_v7();
    w.users.insert(&wallet_user(uid)).await.unwrap();
    let sub = grant_active(&svc, uid).await;
    shop::create_shop(&svc, uid, "data").await.unwrap();
    // Force expiry.
    w.subs
        .set_status(sub.id, SubscriptionStatus::Active, None)
        .await
        .unwrap();
    let mut expired = w.subs.find_by_user(uid).await.unwrap().unwrap();
    expired.expires_at = chrono::Utc::now() - chrono::Duration::days(1);
    w.subs.upsert(&expired).await.unwrap();

    assert!(matches!(
        require_active(&svc, uid).await,
        Err(SubError::Expired)
    ));
    assert!(matches!(
        shop::create_shop(&svc, uid, "new").await,
        Err(ShopError::Subscription(SubError::Expired))
    ));
    // Sweep: active+expired -> grace; data retained.
    let report = run_maintenance(&svc, &svc.payment_config, chrono::Utc::now())
        .await
        .unwrap();
    assert_eq!(report.marked_grace, 1);
    assert_eq!(w.shops.find_for_user(uid).await.unwrap().len(), 1);
}

#[tokio::test]
async fn renewal_within_grace_preserves_data() {
    let (svc, w) = services(bad_config());
    let uid = Uuid::now_v7();
    grant_active(&svc, uid).await;
    shop::create_shop(&svc, uid, "kept").await.unwrap();
    // Expire into grace.
    let mut sub = w.subs.find_by_user(uid).await.unwrap().unwrap();
    sub.expires_at = chrono::Utc::now() - chrono::Duration::days(1);
    w.subs.upsert(&sub).await.unwrap();
    run_maintenance(&svc, &svc.payment_config, chrono::Utc::now())
        .await
        .unwrap();
    assert_eq!(w.shops.find_for_user(uid).await.unwrap().len(), 1);

    // Renew via verified payment.
    let order = subscription::create_order(&svc, uid).await.unwrap();
    let renewed = subscription::verify_order(&svc, w.verifier.as_ref(), uid, order.id, "sig")
        .await
        .unwrap();
    assert_eq!(renewed.status, SubscriptionStatus::Active);
    assert!(renewed.expires_at > chrono::Utc::now());
    assert_eq!(w.shops.find_for_user(uid).await.unwrap().len(), 1);
}

#[tokio::test]
async fn data_deleted_after_grace_and_recovery_window() {
    let (svc, w) = services(bad_config());
    let uid = Uuid::now_v7();
    let other = Uuid::now_v7();
    grant_active(&svc, uid).await;
    grant_active(&svc, other).await;
    shop::create_shop(&svc, uid, "doomed").await.unwrap();
    let other_shop = shop::create_shop(&svc, other, "untouched").await.unwrap();

    let mut sub = w.subs.find_by_user(uid).await.unwrap().unwrap();
    sub.expires_at = chrono::Utc::now() - chrono::Duration::days(1);
    w.subs.upsert(&sub).await.unwrap();
    let report = run_maintenance(&svc, &svc.payment_config, chrono::Utc::now())
        .await
        .unwrap();
    assert_eq!(report.marked_grace, 1);
    // Push past grace (30 days).
    let now = chrono::Utc::now() + chrono::Duration::days(31);
    let report = run_maintenance(&svc, &svc.payment_config, now)
        .await
        .unwrap();
    assert_eq!(report.marked_lapsed, 1);
    assert_eq!(report.soft_deleted_shops, 1);
    // Recovery window (7d) passed -> hard delete (purge_before = now - 7d).
    let purge_time = now + chrono::Duration::days(8);
    let report = run_maintenance(&svc, &svc.payment_config, purge_time)
        .await
        .unwrap();
    assert_eq!(report.hard_deleted_shops, 1);
    assert!(w.shops.find_for_user(uid).await.unwrap().is_empty());
    // Other users untouched.
    assert_eq!(
        w.shops.find_by_id(other_shop.id).await.unwrap().unwrap().id,
        other_shop.id
    );
}

#[tokio::test]
async fn renewal_extends_from_remaining_time() {
    let (svc, _w) = services(bad_config());
    let uid = Uuid::now_v7();
    grant_active(&svc, uid).await;
    let current = svc.subscriptions.find_by_user(uid).await.unwrap().unwrap();
    let base = renewal_base(Some(current.clone()), chrono::Utc::now());
    assert_eq!(base, current.expires_at, "early renewal stacks on expiry");
    let next = add_month(base);
    assert!(next > current.expires_at);
}

#[tokio::test]
async fn concurrent_verify_only_one_wins() {
    let (svc, w) = services(bad_config());
    let uid = Uuid::now_v7();
    let order = subscription::create_order(&svc, uid).await.unwrap();
    let svc1 = svc.clone();
    let v1 = w.verifier.clone();
    let o = order.id;
    let h1 =
        tokio::spawn(
            async move { subscription::verify_order(&svc1, v1.as_ref(), uid, o, "sig").await },
        );
    let svc2 = svc.clone();
    let v2 = w.verifier.clone();
    let h2 =
        tokio::spawn(
            async move { subscription::verify_order(&svc2, v2.as_ref(), uid, o, "sig").await },
        );
    let results = tokio::join!(h1, h2);
    // The atomic pending->paid guard allows exactly one winner.
    let oks = [results.0, results.1]
        .into_iter()
        .map(|r| r.expect("task join ok"))
        .filter(|r| r.is_ok())
        .count();
    assert_eq!(oks, 1, "exactly one concurrent verify may win");
}

#[tokio::test]
async fn employee_role_default_for_wallet_users() {
    let (svc, w) = services(bad_config());
    let (nonce, _m, _e) = auth::nonce(&svc, TEST_WALLET).await.unwrap();
    let verifier = svc.wallet_verifier.clone();
    let tokens = svc.tokens.clone();
    let (uid, _t) = auth::verify(
        &svc,
        verifier.as_ref(),
        tokens.as_ref(),
        TEST_WALLET,
        &nonce,
        GOOD_SIG,
    )
    .await
    .unwrap();
    let user = w.users.find_by_id(uid).await.unwrap().unwrap();
    assert_eq!(user.role, Role::Employee);
}
