//! Round-trip tests: argon2 hash/verify + jwt issue/verify + expiry.

use uuid::Uuid;

use pos_backend::infra::encryption_adapter::hash::Argon2Hasher;
use pos_backend::infra::encryption_adapter::jwt::{Claims, JwtIssuer};
use pos_backend::port::{PasswordHasher, TokenIssuer};

const SECRET: &str = "test-secret";
const PASSWORD: &str = "correct horse battery staple";
const WRONG_PASSWORD: &str = "wrong password entirely";

#[tokio::test]
async fn argon2_hash_verifies_and_rejects_wrong_password() {
    let hasher = Argon2Hasher::new();
    let hash = hasher.hash(PASSWORD).await.expect("valid password hashes");
    assert_ne!(hash, PASSWORD);
    assert!(hasher.verify(PASSWORD, &hash).await.expect("verify ok"));
    assert!(
        !hasher
            .verify(WRONG_PASSWORD, &hash)
            .await
            .expect("verify ok")
    );
}

#[tokio::test]
async fn argon2_salts_produce_distinct_hashes() {
    let hasher = Argon2Hasher::new();
    let a = hasher.hash(PASSWORD).await.expect("hash ok");
    let b = hasher.hash(PASSWORD).await.expect("hash ok");
    assert_ne!(a, b);
}

#[tokio::test]
async fn argon2_rejects_short_password() {
    let hasher = Argon2Hasher::new();
    assert!(hasher.hash("abc").await.is_err());
}

#[tokio::test]
async fn jwt_round_trips_user_id() {
    let issuer = JwtIssuer::new(SECRET.to_owned());
    let id = Uuid::now_v7();
    let token = issuer
        .issue(id, pos_backend::domain::value_objects::Role::Owner)
        .await
        .expect("issue ok");
    assert_eq!(
        issuer.verify(&token).await.expect("verify ok"),
        id,
        "same secret round-trips the user id"
    );
}

#[tokio::test]
async fn jwt_rejects_wrong_secret() {
    let id = Uuid::now_v7();
    let token = JwtIssuer::new(SECRET.to_owned())
        .issue(id, pos_backend::domain::value_objects::Role::Employee)
        .await
        .expect("issue ok");
    let other = JwtIssuer::new("different-secret".to_owned());
    assert!(other.verify(&token).await.is_err());
}

#[tokio::test]
async fn jwt_rejects_tampered_and_expired_tokens() {
    let issuer = JwtIssuer::new(SECRET.to_owned());
    let token = issuer
        .issue(
            Uuid::now_v7(),
            pos_backend::domain::value_objects::Role::Admin,
        )
        .await
        .expect("issue ok");
    let mut tampered = token.clone();
    tampered.replace_range(0..1, "x");
    assert!(issuer.verify(&tampered).await.is_err());

    // Expired claims must fail validation.
    use jsonwebtoken::{DecodingKey, Validation, decode};
    let claims = Claims {
        sub: Uuid::now_v7(),
        role: "Admin".to_owned(),
        exp: 0,
    };
    let expired = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(SECRET.as_bytes()),
    )
    .expect("encode ok");
    assert!(
        decode::<Claims>(
            &expired,
            &DecodingKey::from_secret(SECRET.as_bytes()),
            &Validation::default()
        )
        .is_err()
    );
}
