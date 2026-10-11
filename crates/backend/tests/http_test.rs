//! Router tests against shared in-memory fakes (no DB touched).
#![allow(clippy::todo, clippy::unimplemented)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{RECIPIENT, services, test_config};
use tower::ServiceExt;

fn test_router() -> axum::Router {
    let (svc, _w) = services(test_config(RECIPIENT));
    pos_backend::http_adapter::router::build_router(svc)
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
async fn auth_nonce_with_bad_json_is_400() {
    let res = send(
        Request::builder()
            .method("POST")
            .uri("/auth/nonce")
            .header("content-type", "application/json")
            .body(Body::from("not json"))
            .expect("static request"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn auth_nonce_with_bad_wallet_is_400() {
    let res = send(
        Request::builder()
            .method("POST")
            .uri("/auth/nonce")
            .header("content-type", "application/json")
            .body(Body::from(r#"{ "wallet": "not-base58!!" }"#))
            .expect("static request"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn removed_login_register_routes_are_gone() {
    for uri in ["/auth/login", "/auth/register"] {
        let res = send(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("static request"),
        )
        .await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{uri} removed");
    }
}

#[tokio::test]
async fn subscription_current_requires_auth() {
    let res = send(
        Request::builder()
            .uri("/subscriptions/current")
            .body(Body::empty())
            .expect("static request"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
