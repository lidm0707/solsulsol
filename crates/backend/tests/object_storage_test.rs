//! Object storage port tests: in-memory mock round-trips, DI substitution,
//! and the filesystem adapter round-trip.

mod common;

use common::{MemObjectStorage, ObjectStorage, ObjectStoreError};
use pos_backend::app::config::PaymentConfig;
use pos_backend::app::service::use_cases::AppServices;
use pos_backend::infra::storage_adapter::local::FsObjectStorage;
use pos_backend::port::object_storage::validate_key;
use std::sync::Arc;

const KEY: &str = "shops/0001/billing.pdf";
const OTHER_KEY: &str = "shops/0001/other.pdf";
const CT: &str = "application/pdf";

fn mem() -> MemObjectStorage {
    MemObjectStorage::default()
}

#[tokio::test]
async fn upload_retrieve_delete_round_trip() {
    let s = mem();
    let data = bytes::Bytes::from_static(b"%PDF-1.7 test");
    s.put_object(KEY, data.clone(), CT).await.unwrap();
    let got = s.get_object(KEY).await.unwrap().expect("object stored");
    assert_eq!(got.data, data);
    assert_eq!(got.content_type, CT);
    assert!(s.exists(KEY).await.unwrap());
    s.delete_object(KEY).await.unwrap();
    assert!(!s.exists(KEY).await.unwrap());
    assert!(s.get_object(KEY).await.unwrap().is_none());
}

#[tokio::test]
async fn missing_object_is_none_not_error() {
    let s = mem();
    assert!(s.get_object(OTHER_KEY).await.unwrap().is_none());
    assert!(!s.exists(OTHER_KEY).await.unwrap());
    // Idempotent delete of a missing object.
    s.delete_object(OTHER_KEY).await.unwrap();
}

#[tokio::test]
async fn invalid_keys_are_rejected_before_provider_calls() {
    let s = mem();
    for key in [
        "",
        "/absolute/path",
        "../escape",
        "shops/../secret",
        "a//b",
        "with space",
        "with\\backslash",
        "unicode-é",
        &"x".repeat(513),
    ] {
        let err = s
            .put_object(key, bytes::Bytes::from_static(b"x"), CT)
            .await
            .expect_err("invalid key rejected");
        assert!(matches!(err, ObjectStoreError::InvalidKey(_)), "{key}");
    }
    assert!(s.rows.lock().unwrap().is_empty(), "no provider call made");
}

#[test]
fn validate_key_accepts_healthy_paths() {
    for key in ["a.pdf", "shops/0001/billing.pdf", "a/b/c/d-e_f.tar.gz"] {
        validate_key(key).expect("valid key");
    }
}

fn temp_root() -> String {
    let dir = std::env::temp_dir().join(format!("pos-fs-test-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir.to_string_lossy().into_owned()
}

#[tokio::test]
async fn filesystem_round_trip() {
    let s = FsObjectStorage::new(&temp_root(), Some(1024));
    let data = bytes::Bytes::from_static(b"%PDF-1.7 fs test");
    s.put_object(KEY, data.clone(), CT).await.unwrap();
    let got = s.get_object(KEY).await.unwrap().expect("object stored");
    assert_eq!(got.data, data);
    assert_eq!(got.content_type, CT);
    assert!(s.exists(KEY).await.unwrap());
    s.delete_object(KEY).await.unwrap();
    assert!(!s.exists(KEY).await.unwrap());
    assert!(s.get_object(KEY).await.unwrap().is_none());
}

#[tokio::test]
async fn filesystem_rejects_bad_keys_and_oversize() {
    let s = FsObjectStorage::new(&temp_root(), Some(4));
    assert!(matches!(
        s.put_object("../escape", bytes::Bytes::from_static(b"x"), CT)
            .await,
        Err(ObjectStoreError::InvalidKey(_))
    ));
    assert!(matches!(
        s.put_object(KEY, bytes::Bytes::from_static(b"12345"), CT)
            .await,
        Err(ObjectStoreError::TooLarge { limit: 4 })
    ));
}

/// DI substitution: the app state accepts any `ObjectStorage` implementation
/// (mock here, filesystem in the self-hosted backend, R2 binding on Workers)
/// with no business-logic change.
#[tokio::test]
async fn app_services_accepts_mock_storage() {
    let cfg = PaymentConfig::devnet("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM");
    let (_svc, w) = common::services(cfg.clone());
    let storage = Arc::new(mem());
    let svc = AppServices::builder(cfg)
        .users(w.users.clone())
        .shops(w.shops.clone())
        .members(w.members.clone())
        .goods(w.goods.clone())
        .invites(Arc::new(common::MemInvites::default()))
        .nonces(w.nonces.clone())
        .subscriptions(w.subs.clone())
        .orders(w.orders.clone())
        .storage(w.storage.clone())
        .tokens(Arc::new(common::FixedTokens))
        .wallet_verifier(Arc::new(common::StubVerifier { accept: true }))
        .payment_verifier(w.verifier.clone())
        .destinations(Arc::new(common::MemDestinations))
        .invoices(Arc::new(common::MemInvoices))
        .qr(common::noop_qr())
        .promptpay(common::stub_promptpay())
        .objects(storage.clone())
        .build()
        .expect("services wired with mock storage");
    let storage_port: Arc<dyn ObjectStorage> = svc.objects.expect("objects set");
    storage_port
        .put_object(KEY, bytes::Bytes::from_static(b"hello"), "text/plain")
        .await
        .unwrap();
    assert!(storage_port.exists(KEY).await.unwrap());
    assert!(storage.get_object(KEY).await.unwrap().is_some());
}

#[tokio::test]
async fn app_services_without_storage_is_valid() {
    let cfg = PaymentConfig::devnet("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM");
    let (_svc, w) = common::services(cfg.clone());
    let svc = AppServices::builder(cfg)
        .users(w.users.clone())
        .shops(w.shops.clone())
        .members(w.members.clone())
        .goods(w.goods.clone())
        .invites(Arc::new(common::MemInvites::default()))
        .nonces(w.nonces.clone())
        .subscriptions(w.subs.clone())
        .orders(w.orders.clone())
        .storage(w.storage.clone())
        .tokens(Arc::new(common::FixedTokens))
        .wallet_verifier(Arc::new(common::StubVerifier { accept: true }))
        .payment_verifier(w.verifier.clone())
        .destinations(Arc::new(common::MemDestinations))
        .invoices(Arc::new(common::MemInvoices))
        .qr(common::noop_qr())
        .promptpay(common::stub_promptpay())
        .build()
        .expect("services wired without storage");
    assert!(svc.objects.is_none());
}
