use pos_api::error::ApiError;
use pos_api::schema::Role;

#[test]
fn role_rank_orders_hierarchy() {
    assert!(Role::SuperAdmin.can_manage(Role::Owner));
    assert!(Role::Owner.can_manage(Role::Admin));
    assert!(Role::Admin.can_manage(Role::Employee));
    assert!(!Role::Employee.can_manage(Role::Admin));
    assert!(!Role::Admin.can_manage(Role::Admin));
}

#[test]
fn error_status_maps_to_http() {
    assert_eq!(
        ApiError::Unauthorized.status(),
        axum::http::StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        ApiError::Forbidden.status(),
        axum::http::StatusCode::FORBIDDEN
    );
    assert_eq!(
        ApiError::NotFound.status(),
        axum::http::StatusCode::NOT_FOUND
    );
    assert_eq!(
        ApiError::Conflict("dup".into()).status(),
        axum::http::StatusCode::CONFLICT
    );
    assert_eq!(
        ApiError::Invalid("x".into()).status(),
        axum::http::StatusCode::BAD_REQUEST
    );
    assert_eq!(
        ApiError::Internal.status(),
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[test]
fn schema_round_trips_role_and_chain() {
    let role: Role = serde_json::from_str("\"owner\"").expect("test literal is valid json");
    assert_eq!(role, Role::Owner);
    assert_eq!(
        serde_json::to_string(&role).unwrap_or_default(),
        "\"owner\""
    );

    let chain: pos_api::schema::Chain =
        serde_json::from_str("\"solana\"").expect("test literal is valid json");
    assert_eq!(chain, pos_api::schema::Chain::Solana);
    assert_eq!(
        serde_json::to_string(&chain).unwrap_or_default(),
        "\"solana\""
    );
}
