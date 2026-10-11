//! Shared in-memory fakes for backend tests (no DB, deterministic crypto).
//! Compiled once per test binary; not all binaries use every helper.
#![allow(dead_code, clippy::todo, clippy::unimplemented)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use pos_backend::app::config::PaymentConfig;
use pos_backend::app::service::use_cases::AppServices;
use pos_backend::domain::entities::subscription::{
    AuthNonce, MAX_STORAGE_BYTES, PaymentOrder, PaymentOrderStatus, Subscription,
    SubscriptionStatus,
};
use pos_backend::domain::entities::{
    Billing, Goods, Invite, InviteStatus, PaymentDestination, Shop, ShopMember, User,
};
use pos_backend::domain::value_objects::Role;
// re-exports for sibling test specs; not crate API
pub use pos_backend::port::database::{
    BillingRepo, DestinationRepo, GoodsRepo, InviteRepo, NonceRepo, PaymentOrderRepo,
    ShopMemberRepo, ShopRepo, StorageError, StorageUsageRepo, SubscriptionRepo, UserRepo,
};
pub use pos_backend::port::encryption::TokenIssuer;
pub use pos_backend::port::object_storage::{ObjectStorage, ObjectStoreError, StoredObject};
pub use pos_backend::port::qr::QrRenderer;
pub use pos_backend::port::solana::{
    MessageVerifier, PaymentVerifier, PaymentVerifyError, VerifiedPayment,
};
use rust_decimal::Decimal;
use uuid::Uuid;

pub const TEST_WALLET: &str = "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR";
pub const GOOD_SIG: &str = "good-signature";
pub const RECIPIENT: &str = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
pub const QUOTA: i64 = MAX_STORAGE_BYTES;

/// Signature accepted iff it equals [`GOOD_SIG`].
pub struct StubVerifier {
    pub accept: bool,
}

#[async_trait::async_trait]
impl MessageVerifier for StubVerifier {
    async fn verify(
        &self,
        _message: &str,
        _wallet: &str,
        _signature: &str,
    ) -> Result<bool, String> {
        Ok(self.accept)
    }
}

/// Configurable on-chain verification outcome.
pub enum FakeChain {
    Ok(Decimal),
    NotFound,
    Failed,
    WrongRecipient,
    WrongMint,
    WrongAmount,
}

pub struct StubPaymentVerifier {
    pub outcome: RwLock<FakeChain>,
}

impl StubPaymentVerifier {
    pub fn ok(amount: Decimal) -> Self {
        Self {
            outcome: RwLock::new(FakeChain::Ok(amount)),
        }
    }

    pub fn set(&self, outcome: FakeChain) {
        *self.outcome.write().expect("lock") = outcome;
    }
}

#[async_trait::async_trait]
impl PaymentVerifier for StubPaymentVerifier {
    async fn verify_usdc_transfer(
        &self,
        _signature: &str,
        _recipient: &str,
        _mint: &str,
        _amount: Decimal,
    ) -> Result<VerifiedPayment, PaymentVerifyError> {
        let outcome = self.outcome.read().expect("lock");
        match &*outcome {
            FakeChain::Ok(a) => Ok(VerifiedPayment {
                amount: *a,
                slot: 1,
            }),
            FakeChain::NotFound => Err(PaymentVerifyError::NotFound),
            FakeChain::Failed => Err(PaymentVerifyError::Failed),
            FakeChain::WrongRecipient => Err(PaymentVerifyError::WrongRecipient),
            FakeChain::WrongMint => Err(PaymentVerifyError::WrongMint),
            FakeChain::WrongAmount => Err(PaymentVerifyError::WrongAmount),
        }
    }
}

pub struct FixedTokens;

#[async_trait::async_trait]
impl TokenIssuer for FixedTokens {
    async fn issue(&self, user_id: Uuid, _role: Role) -> Result<String, String> {
        Ok(format!("token-{user_id}"))
    }

    async fn verify(&self, token: &str) -> Result<Uuid, String> {
        token
            .strip_prefix("token-")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| "bad token".to_owned())
    }
}

struct NoopQr;

/// Public constructor for test wiring outside this module.
pub fn noop_qr() -> Arc<dyn QrRenderer> {
    Arc::new(NoopQr)
}

struct StubPromptPay;

/// Public constructor for test wiring outside this module.
pub fn stub_promptpay() -> Arc<dyn pos_backend::port::promptpay::PromptPayEncoder> {
    Arc::new(StubPromptPay)
}

#[async_trait::async_trait]
impl QrRenderer for NoopQr {
    async fn render_png(&self, _payload: &str) -> Result<Vec<u8>, String> {
        Ok(Vec::new())
    }
}

impl pos_backend::port::promptpay::PromptPayEncoder for StubPromptPay {
    fn target(&self, raw: &str) -> String {
        raw.to_owned()
    }

    fn encode_payload(&self, target_id: &str, _amount: Decimal) -> String {
        target_id.to_owned()
    }
}

// ---- in-memory repos ----

#[derive(Default)]
pub struct MemUsers {
    pub rows: Mutex<Vec<User>>,
}

#[async_trait::async_trait]
impl UserRepo for MemUsers {
    async fn insert(&self, u: &User) -> Result<(), String> {
        self.rows.lock().expect("lock").push(u.clone());
        Ok(())
    }
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .find(|u| u.id == id)
            .cloned())
    }
    async fn find_by_wallet(&self, w: &str) -> Result<Option<User>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .find(|u| u.wallet.as_ref().is_some_and(|m| m.as_str() == w))
            .cloned())
    }
}

#[derive(Default)]
pub struct MemShops {
    pub rows: Mutex<Vec<Shop>>,
}

#[async_trait::async_trait]
impl ShopRepo for MemShops {
    async fn insert(&self, s: &Shop) -> Result<(), String> {
        self.rows.lock().expect("lock").push(s.clone());
        Ok(())
    }
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Shop>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .find(|s| s.id == id)
            .cloned())
    }
    async fn find_for_user(&self, user_id: Uuid) -> Result<Vec<Shop>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .filter(|s| s.owner_id == user_id && s.deleted_at.is_none())
            .cloned()
            .collect())
    }
    async fn soft_delete_for_owner(
        &self,
        owner_id: Uuid,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, String> {
        let mut rows = self.rows.lock().expect("lock");
        let mut n = 0;
        for s in rows.iter_mut().filter(|s| s.owner_id == owner_id) {
            s.deleted_at = Some(now);
            n += 1;
        }
        Ok(n)
    }
    async fn hard_delete_expired(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, String> {
        let mut rows = self.rows.lock().expect("lock");
        let before_count = rows.len();
        rows.retain(|s| !s.deleted_at.is_some_and(|d| d < before));
        Ok((before_count - rows.len()) as u64)
    }
}

#[derive(Default)]
pub struct MemMembers {
    pub rows: Mutex<Vec<ShopMember>>,
}

#[async_trait::async_trait]
impl ShopMemberRepo for MemMembers {
    async fn insert(&self, m: &ShopMember) -> Result<(), String> {
        self.rows.lock().expect("lock").push(m.clone());
        Ok(())
    }
    async fn find_role(&self, s: Uuid, u: Uuid) -> Result<Option<Role>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .find(|m| m.shop_id == s && m.user_id == u)
            .map(|m| m.role))
    }
    async fn list_by_shop(&self, s: Uuid) -> Result<Vec<ShopMember>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .filter(|m| m.shop_id == s)
            .cloned()
            .collect())
    }
    async fn update_role(&self, s: Uuid, u: Uuid, r: Role) -> Result<(), String> {
        let mut rows = self.rows.lock().expect("lock");
        if let Some(m) = rows.iter_mut().find(|m| m.shop_id == s && m.user_id == u) {
            m.role = r;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct MemGoods {
    pub rows: Mutex<Vec<Goods>>,
}

#[async_trait::async_trait]
impl GoodsRepo for MemGoods {
    async fn insert(&self, p: &Goods) -> Result<(), String> {
        self.rows.lock().expect("lock").push(p.clone());
        Ok(())
    }
    async fn find_by_shop(&self, s: Uuid) -> Result<Vec<Goods>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .filter(|p| p.shop_id == s)
            .cloned()
            .collect())
    }
    async fn take_units(&self, _p: Uuid) -> Result<(), String> {
        unimplemented!("not exercised")
    }
    async fn find_by_id(&self, goods_id: Uuid) -> Result<Option<Goods>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .find(|p| p.id == goods_id)
            .cloned())
    }
    async fn delete(&self, goods_id: Uuid) -> Result<(), String> {
        self.rows.lock().expect("lock").retain(|p| p.id != goods_id);
        Ok(())
    }
}

#[derive(Default)]
pub struct MemDestinations;

#[async_trait::async_trait]
impl DestinationRepo for MemDestinations {
    async fn insert(&self, _d: &PaymentDestination) -> Result<(), String> {
        unimplemented!("not exercised")
    }
    async fn find_by_id(&self, _id: Uuid) -> Result<Option<PaymentDestination>, String> {
        unimplemented!("not exercised")
    }
    async fn list_by_shop(&self, _s: Uuid) -> Result<Vec<PaymentDestination>, String> {
        unimplemented!("not exercised")
    }
    async fn delete(&self, _id: Uuid) -> Result<(), String> {
        unimplemented!("not exercised")
    }
}

#[derive(Default)]
pub struct MemInvites {
    pub rows: Mutex<Vec<Invite>>,
}

#[async_trait::async_trait]
impl InviteRepo for MemInvites {
    async fn insert(&self, i: &Invite) -> Result<(), String> {
        self.rows.lock().expect("lock").push(i.clone());
        Ok(())
    }
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Invite>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .find(|i| i.id == id)
            .cloned())
    }
    async fn list_by_shop(&self, _s: Uuid) -> Result<Vec<Invite>, String> {
        unimplemented!("not exercised")
    }
    async fn set_status(&self, id: Uuid, st: InviteStatus) -> Result<(), String> {
        let mut rows = self.rows.lock().expect("lock");
        if let Some(i) = rows.iter_mut().find(|i| i.id == id) {
            i.status = st;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct MemBillings;

#[async_trait::async_trait]
impl BillingRepo for MemBillings {
    async fn insert(&self, _i: &Billing) -> Result<(), String> {
        unimplemented!("not exercised")
    }
    async fn find_by_id(&self, _id: Uuid) -> Result<Option<Billing>, String> {
        unimplemented!("not exercised")
    }
    async fn list_by_shop(&self, _s: Uuid) -> Result<Vec<Billing>, String> {
        unimplemented!("not exercised")
    }
    async fn sales_by_goods(&self, _s: Uuid) -> Result<Vec<(Uuid, u64)>, String> {
        unimplemented!("not exercised")
    }
    async fn goods_has_lines(&self, _g: Uuid) -> Result<bool, String> {
        Ok(false)
    }
    async fn hard_delete_expired(
        &self,
        _before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, String> {
        // Mem billings are not cascade-linked to mem shops; the real
        // billings-first purge is covered by purge_pg_test.
        Ok(0)
    }
}

#[derive(Default)]
pub struct MemNonces {
    pub rows: Mutex<Vec<(AuthNonce, Option<chrono::DateTime<chrono::Utc>>)>>,
}

#[async_trait::async_trait]
impl NonceRepo for MemNonces {
    async fn insert(&self, n: &AuthNonce) -> Result<(), String> {
        self.rows.lock().expect("lock").push((n.clone(), None));
        Ok(())
    }
    async fn find(&self, wallet: &str, nonce: &str) -> Result<Option<AuthNonce>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .find(|(n, used)| n.wallet == wallet && n.nonce == nonce && used.is_none())
            .map(|(n, _)| n.clone()))
    }
    async fn consume(
        &self,
        wallet: &str,
        nonce: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<bool, String> {
        let mut rows = self.rows.lock().expect("lock");
        if let Some(slot) = rows.iter_mut().find(|(n, used)| {
            n.wallet == wallet && n.nonce == nonce && used.is_none() && n.expires_at > now
        }) {
            slot.1 = Some(now);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[derive(Default)]
pub struct MemSubs {
    pub rows: Mutex<Vec<Subscription>>,
}

#[async_trait::async_trait]
impl SubscriptionRepo for MemSubs {
    async fn find_by_user(&self, u: Uuid) -> Result<Option<Subscription>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .find(|s| s.user_id == u)
            .cloned())
    }
    async fn upsert(&self, sub: &Subscription) -> Result<(), String> {
        let mut rows = self.rows.lock().expect("lock");
        if let Some(s) = rows.iter_mut().find(|s| s.user_id == sub.user_id) {
            *s = sub.clone();
        } else {
            rows.push(sub.clone());
        }
        Ok(())
    }
    async fn list_to_mark_grace(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Subscription>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .filter(|s| s.status == SubscriptionStatus::Active && s.expires_at <= now)
            .cloned()
            .collect())
    }
    async fn list_to_mark_lapsed(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Subscription>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .filter(|s| {
                s.status == SubscriptionStatus::Grace && s.grace_until.is_some_and(|g| g <= now)
            })
            .cloned()
            .collect())
    }
    async fn list_to_delete(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Subscription>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .filter(|s| {
                s.status != SubscriptionStatus::Active
                    && s.status != SubscriptionStatus::Grace
                    && s.grace_until.is_some_and(|g| g <= before)
            })
            .cloned()
            .collect())
    }
    async fn set_status(
        &self,
        id: Uuid,
        status: SubscriptionStatus,
        grace_until: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), String> {
        let mut rows = self.rows.lock().expect("lock");
        if let Some(s) = rows.iter_mut().find(|s| s.id == id) {
            s.status = status;
            s.grace_until = grace_until;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct MemOrders {
    pub rows: Mutex<Vec<PaymentOrder>>,
}

#[async_trait::async_trait]
impl pos_backend::port::database::PaymentOrderRepo for MemOrders {
    async fn insert(&self, o: &PaymentOrder) -> Result<(), String> {
        self.rows.lock().expect("lock").push(o.clone());
        Ok(())
    }
    async fn find_by_id(&self, id: Uuid) -> Result<Option<PaymentOrder>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .find(|o| o.id == id)
            .cloned())
    }
    async fn find_by_signature(&self, signature: &str) -> Result<Option<PaymentOrder>, String> {
        Ok(self
            .rows
            .lock()
            .expect("lock")
            .iter()
            .find(|o| o.signature.as_deref() == Some(signature))
            .cloned())
    }
    async fn mark_paid_if_pending(
        &self,
        id: Uuid,
        signature: &str,
        paid_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<bool, String> {
        let mut rows = self.rows.lock().expect("lock");
        if rows.iter().any(|o| {
            o.id != id
                && o.signature.as_deref() == Some(signature)
                && o.status == PaymentOrderStatus::Paid
        }) {
            return Ok(false);
        }
        if let Some(o) = rows
            .iter_mut()
            .find(|o| o.id == id && o.status == PaymentOrderStatus::Pending)
        {
            o.status = PaymentOrderStatus::Paid;
            o.signature = Some(signature.to_owned());
            o.paid_at = Some(paid_at);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[derive(Default)]
pub struct MemStorage {
    pub rows: Mutex<HashMap<(Uuid, Uuid), i64>>,
}

#[async_trait::async_trait]
impl StorageUsageRepo for MemStorage {
    async fn add_checked(
        &self,
        shop_id: Uuid,
        key: Uuid,
        bytes: i64,
        quota: i64,
    ) -> Result<i64, StorageError> {
        let mut rows = self.rows.lock().expect("lock");
        let known = rows.contains_key(&(shop_id, key));
        if !known {
            let prospective: i64 = rows
                .iter()
                .filter(|((s, _), _)| *s == shop_id)
                .map(|(_, b)| b)
                .sum::<i64>()
                + bytes;
            if prospective > quota {
                return Err(StorageError::QuotaExceeded);
            }
            rows.insert((shop_id, key), bytes);
        }
        let total: i64 = rows
            .iter()
            .filter(|((s, _), _)| *s == shop_id)
            .map(|(_, b)| b)
            .sum();
        Ok(total)
    }

    async fn total(&self, shop_id: Uuid) -> Result<i64, StorageError> {
        let rows = self.rows.lock().expect("lock");
        Ok(rows
            .iter()
            .filter(|((s, _), _)| *s == shop_id)
            .map(|(_, b)| b)
            .sum())
    }

    async fn list(&self, shop_id: Uuid) -> Result<Vec<(Uuid, i64)>, StorageError> {
        let rows = self.rows.lock().expect("lock");
        let mut items: Vec<(Uuid, i64)> = rows
            .iter()
            .filter(|((s, _), _)| *s == shop_id)
            .map(|((_, k), b)| (*k, *b))
            .collect();
        items.sort_by_key(|(k, _)| *k);
        Ok(items)
    }

    async fn remove(&self, shop_id: Uuid, key: Uuid) -> Result<i64, StorageError> {
        let mut rows = self.rows.lock().expect("lock");
        rows.remove(&(shop_id, key));
        Ok(rows
            .iter()
            .filter(|((s, _), _)| *s == shop_id)
            .map(|(_, b)| b)
            .sum())
    }
}

// ---- wiring ----

pub fn test_config(recipient: &str) -> PaymentConfig {
    PaymentConfig {
        domain: "test.local".to_owned(),
        recipient: recipient.to_owned(),
        ..PaymentConfig::devnet(recipient)
    }
}

pub struct SvcWiring {
    pub users: Arc<MemUsers>,
    pub shops: Arc<MemShops>,
    pub members: Arc<MemMembers>,
    pub goods: Arc<MemGoods>,
    pub subs: Arc<MemSubs>,
    pub orders: Arc<MemOrders>,
    pub storage: Arc<MemStorage>,
    pub nonces: Arc<MemNonces>,
    pub verifier: Arc<StubPaymentVerifier>,
}

pub fn services(cfg: PaymentConfig) -> (AppServices, SvcWiring) {
    let users = Arc::new(MemUsers::default());
    let shops = Arc::new(MemShops::default());
    let members = Arc::new(MemMembers::default());
    let goods = Arc::new(MemGoods::default());
    let subs = Arc::new(MemSubs::default());
    let orders = Arc::new(MemOrders::default());
    let storage = Arc::new(MemStorage::default());
    let nonces = Arc::new(MemNonces::default());
    let verifier = Arc::new(StubPaymentVerifier::ok(Decimal::ONE));
    let svc = AppServices::builder(cfg)
        .users(users.clone())
        .shops(shops.clone())
        .members(members.clone())
        .goods(goods.clone())
        .destinations(Arc::new(MemDestinations))
        .invites(Arc::new(MemInvites::default()))
        .billings(Arc::new(MemBillings))
        .nonces(nonces.clone())
        .subscriptions(subs.clone())
        .orders(orders.clone())
        .storage(storage.clone())
        .tokens(Arc::new(FixedTokens))
        .wallet_verifier(Arc::new(StubVerifier { accept: true }))
        .payment_verifier(verifier.clone())
        .qr(Arc::new(NoopQr))
        .promptpay(Arc::new(StubPromptPay))
        .build()
        .expect("services wired");
    (
        svc,
        SvcWiring {
            users,
            shops,
            members,
            goods,
            subs,
            orders,
            storage,
            nonces,
            verifier,
        },
    )
}

pub fn wallet_user(id: Uuid) -> User {
    User {
        id,
        email: None,
        wallet: Some(
            pos_backend::domain::value_objects::Wallet::parse(TEST_WALLET)
                .expect("test wallet is valid"),
        ),
        role: Role::Employee,
        created_at: chrono::Utc::now(),
    }
}

/// Inserts an active subscription for `user_id` starting now.
pub async fn grant_active(svc: &AppServices, user_id: Uuid) -> Subscription {
    let now = chrono::Utc::now();
    let sub = Subscription {
        id: Uuid::now_v7(),
        user_id,
        status: SubscriptionStatus::Active,
        started_at: now,
        expires_at: now + chrono::Duration::days(30),
        grace_until: None,
    };
    svc.subscriptions.upsert(&sub).await.expect("sub upsert");
    sub
}

/// In-memory [`ObjectStorage`] mock: lets services be tested without any
/// S3/R2 provider.
#[derive(Default)]
pub struct MemObjectStorage {
    pub rows: Mutex<HashMap<String, StoredObject>>,
}

#[async_trait::async_trait]
impl ObjectStorage for MemObjectStorage {
    async fn put_object(
        &self,
        key: &str,
        data: bytes::Bytes,
        content_type: &str,
    ) -> Result<(), ObjectStoreError> {
        pos_backend::port::object_storage::validate_key(key)?;
        self.rows.lock().expect("lock").insert(
            key.to_owned(),
            StoredObject {
                data,
                content_type: content_type.to_owned(),
            },
        );
        Ok(())
    }

    async fn get_object(&self, key: &str) -> Result<Option<StoredObject>, ObjectStoreError> {
        pos_backend::port::object_storage::validate_key(key)?;
        Ok(self.rows.lock().expect("lock").get(key).cloned())
    }

    async fn delete_object(&self, key: &str) -> Result<(), ObjectStoreError> {
        pos_backend::port::object_storage::validate_key(key)?;
        self.rows.lock().expect("lock").remove(key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, ObjectStoreError> {
        pos_backend::port::object_storage::validate_key(key)?;
        Ok(self.rows.lock().expect("lock").contains_key(key))
    }
}
