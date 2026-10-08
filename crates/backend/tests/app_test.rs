//! App-layer use-case tests with mockall repo mocks.
#![allow(clippy::todo, clippy::unimplemented)]

use mockall::mock;
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

use pos_backend::app::service::shop::{self, ShopError};
use pos_backend::domain::entities::{
    Invite, InviteStatus, PaymentDestination, Product, Shop, ShopMember, User,
};
use pos_backend::domain::value_objects::{Chain, ChainAddress, Email, Money, Role, SplToken};

mock! {
    UserRepo {}
    #[async_trait::async_trait]
    impl pos_backend::port::UserRepo for UserRepo {
        async fn insert(&self, user: &User) -> Result<(), String>;
        async fn find_by_email(&self, email: &str) -> Result<Option<User>, String>;
        async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, String>;
    }
}

mock! {
    InviteRepo { }
    #[async_trait::async_trait]
    impl pos_backend::port::InviteRepo for InviteRepo {
        async fn insert(&self, invite: &Invite) -> Result<(), String>;
        async fn find_by_id(&self, id: Uuid) -> Result<Option<Invite>, String>;
        async fn list_by_shop(&self, shop_id: Uuid) -> Result<Vec<Invite>, String>;
        async fn set_status(&self, id: Uuid, status: InviteStatus) -> Result<(), String>;
    }
}

mock! {
    DestinationRepo { }
    #[async_trait::async_trait]
    impl pos_backend::port::DestinationRepo for DestinationRepo {
        async fn insert(&self, d: &PaymentDestination) -> Result<(), String>;
        async fn find_by_id(&self, id: Uuid) -> Result<Option<PaymentDestination>, String>;
        async fn list_by_shop(&self, shop_id: Uuid) -> Result<Vec<PaymentDestination>, String>;
        async fn delete(&self, id: Uuid) -> Result<(), String>;
    }
}

const SOL: &str = "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR";

fn user(id: Uuid, email: &str, role: Role) -> User {
    User {
        id,
        email: Email::parse(email).expect("test email is valid"),
        password: pos_backend::domain::value_objects::PasswordHash::from_hash("h".to_owned()),
        role,
        created_at: chrono::Utc::now(),
    }
}

fn svc(
    members: MockMemberRepo2,
    users: MockUserRepo,
    invites: MockInviteRepo,
    destinations: MockDestinationRepo,
) -> pos_backend::app::service::use_cases::AppServices {
    pos_backend::app::service::use_cases::AppServices::builder()
        .users(Arc::new(users))
        .shops(Arc::new(NoopShopRepo))
        .members(Arc::new(members))
        .products(Arc::new(NoopProductRepo))
        .destinations(Arc::new(destinations))
        .invites(Arc::new(invites))
        .invoices(Arc::new(NoopInvoiceRepo))
        .hasher(Arc::new(NoopHasher))
        .tokens(Arc::new(NoopTokens))
        .qr(Arc::new(NoopQr))
        .promptpay(Arc::new(NoopPromptPay))
        .build()
        .expect("all services wired")
}

macro_rules! noop {
    ($name:ident, $trait:ty) => {
        struct $name;
    };
}

noop!(NoopShopRepo, pos_backend::port::ShopRepo);
noop!(NoopProductRepo, pos_backend::port::ProductRepo);
noop!(NoopInvoiceRepo, pos_backend::port::InvoiceRepo);
noop!(NoopHasher, pos_backend::port::PasswordHasher);
noop!(NoopTokens, pos_backend::port::TokenIssuer);
noop!(NoopQr, pos_backend::port::QrRenderer);
noop!(NoopPromptPay, pos_backend::port::PromptPayEncoder);

impl pos_backend::port::PromptPayEncoder for NoopPromptPay {
    fn target(&self, raw: &str) -> String {
        raw.to_owned()
    }

    fn encode_payload(&self, target_id: &str, _amount: rust_decimal::Decimal) -> String {
        target_id.to_owned()
    }
}

#[async_trait::async_trait]
impl pos_backend::port::ShopRepo for NoopShopRepo {
    async fn insert(&self, _shop: &Shop) -> Result<(), String> {
        unimplemented!("not used in this test")
    }
    async fn find_by_id(&self, _id: Uuid) -> Result<Option<Shop>, String> {
        unimplemented!("not used in this test")
    }
    async fn find_for_user(&self, _user_id: Uuid) -> Result<Vec<Shop>, String> {
        unimplemented!("not used in this test")
    }
}

#[async_trait::async_trait]
impl pos_backend::port::ProductRepo for NoopProductRepo {
    async fn insert(&self, _product: &Product) -> Result<(), String> {
        unimplemented!("not used in this test")
    }
    async fn find_by_shop(&self, _shop_id: Uuid) -> Result<Vec<Product>, String> {
        unimplemented!("not used in this test")
    }
    async fn take_units(&self, _product_id: Uuid) -> Result<(), String> {
        unimplemented!("not used in this test")
    }
}

#[async_trait::async_trait]
impl pos_backend::port::InvoiceRepo for NoopInvoiceRepo {
    async fn insert(
        &self,
        _invoice: &pos_backend::domain::entities::Invoice,
    ) -> Result<(), String> {
        unimplemented!("not used in this test")
    }
    async fn find_by_id(
        &self,
        _id: Uuid,
    ) -> Result<Option<pos_backend::domain::entities::Invoice>, String> {
        unimplemented!("not used in this test")
    }
    async fn list_by_shop(
        &self,
        _shop_id: Uuid,
    ) -> Result<Vec<pos_backend::domain::entities::Invoice>, String> {
        unimplemented!("not used in this test")
    }
}

#[async_trait::async_trait]
impl pos_backend::port::PasswordHasher for NoopHasher {
    async fn hash(&self, _password: &str) -> Result<String, String> {
        unimplemented!("not used in this test")
    }
    async fn verify(&self, _password: &str, _hash: &str) -> Result<bool, String> {
        unimplemented!("not used in this test")
    }
}

#[async_trait::async_trait]
impl pos_backend::port::TokenIssuer for NoopTokens {
    async fn issue(&self, _user_id: Uuid, _role: Role) -> Result<String, String> {
        unimplemented!("not used in this test")
    }
    async fn verify(&self, _token: &str) -> Result<Uuid, String> {
        unimplemented!("not used in this test")
    }
}

#[async_trait::async_trait]
impl pos_backend::port::QrRenderer for NoopQr {
    async fn render_png(&self, _payload: &str) -> Result<Vec<u8>, String> {
        unimplemented!("not used in this test")
    }
}

// mock! with `implements SomeAlias` is not how mockall works;
// declare the member mock explicitly.
mock! {
    MemberRepo2 { }
    #[async_trait::async_trait]
    impl pos_backend::port::ShopMemberRepo for MemberRepo2 {
        async fn insert(&self, member: &ShopMember) -> Result<(), String>;
        async fn find_role(&self, shop_id: Uuid, user_id: Uuid) -> Result<Option<Role>, String>;
        async fn list_by_shop(&self, shop_id: Uuid) -> Result<Vec<ShopMember>, String>;
        async fn update_role(&self, shop_id: Uuid, user_id: Uuid, role: Role) -> Result<(), String>;
    }
}

mock! {
    ProductRepo { }
    #[async_trait::async_trait]
    impl pos_backend::port::ProductRepo for ProductRepo {
        async fn insert(&self, product: &Product) -> Result<(), String>;
        async fn find_by_shop(&self, shop_id: Uuid) -> Result<Vec<Product>, String>;
        async fn take_units(&self, product_id: Uuid) -> Result<(), String>;
    }
}

const SHOP: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000001");

#[tokio::test]
async fn admin_can_invite_employee_but_not_owner() {
    let admin_id = Uuid::now_v7();
    let mut members = MockMemberRepo2::new();
    members
        .expect_find_role()
        .withf(move |_, uid| *uid == admin_id)
        .returning(|_, _| Ok(Some(Role::Admin)));
    let mut users = MockUserRepo::new();
    users
        .expect_find_by_email()
        .returning(|_| Ok(Some(user(Uuid::now_v7(), "e@x.com", Role::Employee))));
    let mut invites = MockInviteRepo::new();
    invites.expect_insert().returning(|_| Ok(()));

    let s = svc(members, users, invites, MockDestinationRepo::new());
    let invite = shop::invite(&s, admin_id, SHOP, "e@x.com", Role::Employee)
        .await
        .expect("admin invites employee");
    assert_eq!(invite.status, InviteStatus::Pending);

    let mut members2 = MockMemberRepo2::new();
    members2
        .expect_find_role()
        .withf(move |_, uid| *uid == admin_id)
        .returning(|_, _| Ok(Some(Role::Admin)));
    let s2 = svc(
        members2,
        MockUserRepo::new(),
        MockInviteRepo::new(),
        MockDestinationRepo::new(),
    );
    assert!(matches!(
        shop::invite(&s2, admin_id, SHOP, "e@x.com", Role::Owner).await,
        Err(ShopError::Rule(_))
    ));
}

#[tokio::test]
async fn destination_requires_admin() {
    let emp_id = Uuid::now_v7();
    let mut members = MockMemberRepo2::new();
    members
        .expect_find_role()
        .withf(move |_, uid| *uid == emp_id)
        .returning(|_, _| Ok(Some(Role::Employee)));
    let s = svc(
        members,
        MockUserRepo::new(),
        MockInviteRepo::new(),
        MockDestinationRepo::new(),
    );
    let addr = ChainAddress::parse(Chain::Solana, SOL).expect("test sol address is valid");
    assert!(matches!(
        pos_backend::app::service::destination::add_destination(&s, emp_id, SHOP, addr, SplToken::Sol).await,
        Err(ShopError::Forbidden)
    ));
}

#[tokio::test]
async fn goods_round_trip_with_admin_role() {
    let admin_id = Uuid::now_v7();
    let mut members = MockMemberRepo2::new();
    members
        .expect_find_role()
        .withf(move |_, uid| *uid == admin_id)
        .returning(|_, _| Ok(Some(Role::Admin)));
    let mut products = MockProductRepo::new();
    products.expect_insert().returning(|_| Ok(()));
    products.expect_find_by_shop().returning(|_| Ok(Vec::new()));

    let s = pos_backend::app::service::use_cases::AppServices::builder()
        .users(Arc::new(MockUserRepo::new()))
        .shops(Arc::new(NoopShopRepo))
        .members(Arc::new(members))
        .products(Arc::new(products))
        .destinations(Arc::new(MockDestinationRepo::new()))
        .invites(Arc::new(MockInviteRepo::new()))
        .invoices(Arc::new(NoopInvoiceRepo))
        .hasher(Arc::new(NoopHasher))
        .tokens(Arc::new(NoopTokens))
        .qr(Arc::new(NoopQr))
        .promptpay(Arc::new(NoopPromptPay))
        .build()
        .expect("all services wired");

    let money = Money::new(Decimal::from(5)).expect("test amount is valid");
    let cost = Decimal::from(2);
    let units = 10;
    let barcode = Some(String::from("4006381333931"));
    let p = pos_backend::app::service::goods::add_goods(
        &s,
        admin_id,
        SHOP,
        pos_backend::app::service::goods::NewGoods {
            name: "coffee",
            price: money,
            cost,
            units,
            barcode,
        },
    )
    .await
    .expect("admin adds goods");
    assert_eq!(p.name, "coffee");
    assert_eq!(p.cost, cost);
    assert_eq!(p.units, units);
    assert_eq!(p.barcode.as_deref(), Some("4006381333931"));
    let listed = pos_backend::app::service::goods::list_goods(&s, admin_id, SHOP)
        .await
        .expect("lists goods");
    assert!(listed.is_empty());
}
