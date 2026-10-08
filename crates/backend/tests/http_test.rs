//! Router tests against in-file no-op services (no DB touched on these paths).
#![allow(clippy::todo, clippy::unimplemented)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::sync::Arc;
use tower::ServiceExt;

use pos_backend::app::service::use_cases::AppServices;
use pos_backend::domain::entities::{
    Invite, InviteStatus, Invoice, PaymentDestination, Product, Shop, ShopMember, User,
};
use pos_backend::domain::value_objects::Role;
use uuid::Uuid;

struct AlwaysErr;
macro_rules! unimpl {
    () => {
        unimplemented!("not exercised by these tests")
    };
}

#[async_trait::async_trait]
impl pos_backend::port::UserRepo for AlwaysErr {
    async fn insert(&self, _u: &User) -> Result<(), String> {
        unimpl!()
    }
    async fn find_by_email(&self, _e: &str) -> Result<Option<User>, String> {
        unimpl!()
    }
    async fn find_by_id(&self, _id: Uuid) -> Result<Option<User>, String> {
        unimpl!()
    }
}

#[async_trait::async_trait]
impl pos_backend::port::ShopRepo for AlwaysErr {
    async fn insert(&self, _s: &Shop) -> Result<(), String> {
        unimpl!()
    }
    async fn find_by_id(&self, _id: Uuid) -> Result<Option<Shop>, String> {
        unimpl!()
    }
    async fn find_for_user(&self, _u: Uuid) -> Result<Vec<Shop>, String> {
        unimpl!()
    }
}

#[async_trait::async_trait]
impl pos_backend::port::ShopMemberRepo for AlwaysErr {
    async fn insert(&self, _m: &ShopMember) -> Result<(), String> {
        unimpl!()
    }
    async fn find_role(&self, _s: Uuid, _u: Uuid) -> Result<Option<Role>, String> {
        unimpl!()
    }
    async fn list_by_shop(&self, _s: Uuid) -> Result<Vec<ShopMember>, String> {
        unimpl!()
    }
    async fn update_role(&self, _s: Uuid, _u: Uuid, _r: Role) -> Result<(), String> {
        unimpl!()
    }
}

#[async_trait::async_trait]
impl pos_backend::port::ProductRepo for AlwaysErr {
    async fn insert(&self, _p: &Product) -> Result<(), String> {
        unimpl!()
    }
    async fn find_by_shop(&self, _s: Uuid) -> Result<Vec<Product>, String> {
        unimpl!()
    }
    async fn take_units(&self, _p: Uuid) -> Result<(), String> {
        unimpl!()
    }
}

#[async_trait::async_trait]
impl pos_backend::port::DestinationRepo for AlwaysErr {
    async fn insert(&self, _d: &PaymentDestination) -> Result<(), String> {
        unimpl!()
    }
    async fn find_by_id(&self, _id: Uuid) -> Result<Option<PaymentDestination>, String> {
        unimpl!()
    }
    async fn list_by_shop(&self, _s: Uuid) -> Result<Vec<PaymentDestination>, String> {
        unimpl!()
    }
    async fn delete(&self, _id: Uuid) -> Result<(), String> {
        unimpl!()
    }
}

#[async_trait::async_trait]
impl pos_backend::port::InviteRepo for AlwaysErr {
    async fn insert(&self, _i: &Invite) -> Result<(), String> {
        unimpl!()
    }
    async fn find_by_id(&self, _id: Uuid) -> Result<Option<Invite>, String> {
        unimpl!()
    }
    async fn list_by_shop(&self, _s: Uuid) -> Result<Vec<Invite>, String> {
        unimpl!()
    }
    async fn set_status(&self, _id: Uuid, _st: InviteStatus) -> Result<(), String> {
        unimpl!()
    }
}

#[async_trait::async_trait]
impl pos_backend::port::InvoiceRepo for AlwaysErr {
    async fn insert(&self, _i: &Invoice) -> Result<(), String> {
        unimpl!()
    }
    async fn find_by_id(&self, _id: Uuid) -> Result<Option<Invoice>, String> {
        unimpl!()
    }
    async fn list_by_shop(&self, _s: Uuid) -> Result<Vec<Invoice>, String> {
        unimpl!()
    }
}

#[async_trait::async_trait]
impl pos_backend::port::PasswordHasher for AlwaysErr {
    async fn hash(&self, _p: &str) -> Result<String, String> {
        unimpl!()
    }
    async fn verify(&self, _p: &str, _h: &str) -> Result<bool, String> {
        unimpl!()
    }
}

#[async_trait::async_trait]
impl pos_backend::port::TokenIssuer for AlwaysErr {
    async fn issue(&self, _u: Uuid, _r: Role) -> Result<String, String> {
        unimpl!()
    }
    async fn verify(&self, _t: &str) -> Result<Uuid, String> {
        Err(String::from("no tokens in this test"))
    }
}

#[async_trait::async_trait]
impl pos_backend::port::QrRenderer for AlwaysErr {
    async fn render_png(&self, _p: &str) -> Result<Vec<u8>, String> {
        unimpl!()
    }
}

impl pos_backend::port::PromptPayEncoder for AlwaysErr {
    fn target(&self, _raw: &str) -> String {
        unimpl!()
    }

    fn encode_payload(&self, _target_id: &str, _amount: rust_decimal::Decimal) -> String {
        unimpl!()
    }
}

fn test_router() -> axum::Router {
    let svc = AppServices::builder()
        .users(Arc::new(AlwaysErr))
        .shops(Arc::new(AlwaysErr))
        .members(Arc::new(AlwaysErr))
        .products(Arc::new(AlwaysErr))
        .destinations(Arc::new(AlwaysErr))
        .invites(Arc::new(AlwaysErr))
        .invoices(Arc::new(AlwaysErr))
        .hasher(Arc::new(AlwaysErr))
        .tokens(Arc::new(AlwaysErr))
        .qr(Arc::new(AlwaysErr))
        .promptpay(Arc::new(AlwaysErr))
        .build()
        .expect("all services wired");
    pos_backend::infra::http_adapter::router::build_router(svc)
}

async fn send(req: Request<Body>) -> axum::response::Response {
    test_router()
        .oneshot(req)
        .await
        .expect("infallible oneshot")
}

#[tokio::test]
async fn health_is_public() {
    let res = send(
        Request::builder()
            .uri("/health")
            .body(Body::empty())
            .expect("static request"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn protected_route_requires_token() {
    let res = send(
        Request::builder()
            .uri("/shops")
            .body(Body::empty())
            .expect("static request"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn register_with_bad_json_is_400() {
    let res = send(
        Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header("content-type", "application/json")
            .body(Body::from("not json"))
            .expect("static request"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
