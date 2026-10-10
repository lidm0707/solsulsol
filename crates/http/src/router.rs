//! Routes. State is `AppServices` directly (Clone).

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use pos_api::error::ApiError;
use pos_api::routes;
use pos_api::schema::{
    AddDestinationRequest, AddGoodsRequest, AuthNonceRequest, AuthNonceResponse, AuthResponse,
    AuthVerifyRequest, CheckoutRequest, CreatePaymentOrderRequest, CreateShopRequest, Currency,
    DestinationResponse, GoodsResponse, GoodsSalesResponse, InviteAcceptRequest, InviteRequest,
    InviteResponse, MeResponse, PaymentOrderResponse, PaymentOrderStatusApi, ShopMemberResponse,
    ShopResponse, SubscriptionResponse, SubscriptionStatusApi, UpdateMemberRoleRequest,
    UpdateShopSettingsRequest, VatMode, VerifyPaymentRequest,
};
use tower_http::cors::CorsLayer;
#[cfg(not(target_arch = "wasm32"))]
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::error::ApiErrorReponse;
use crate::extract::AuthUser;
use pos_app::domain::entities::subscription::{
    MAX_GOODS_PER_SHOP, MAX_SHOPS_PER_USER, MAX_STORAGE_BYTES, SUB_PRICE_USDC,
};
use pos_app::domain::value_objects::{ChainAddress, Email, Money, Wallet};
use pos_app::service::destination as dest_svc;
use pos_app::service::goods as goods_svc;
use pos_app::service::shop::{self, ShopError};
use pos_app::service::subscription as sub_svc;
use pos_app::service::use_cases::AppServices;

type Api<T> = Result<axum::Json<T>, ApiErrorReponse>;

const fn role_to_api(r: pos_app::domain::value_objects::Role) -> pos_api::schema::Role {
    match r {
        pos_app::domain::value_objects::Role::Employee => pos_api::schema::Role::Employee,
        pos_app::domain::value_objects::Role::Admin => pos_api::schema::Role::Admin,
        pos_app::domain::value_objects::Role::Owner => pos_api::schema::Role::Owner,
        pos_app::domain::value_objects::Role::SuperAdmin => pos_api::schema::Role::SuperAdmin,
    }
}

const fn role_to_domain(r: pos_api::schema::Role) -> pos_app::domain::value_objects::Role {
    match r {
        pos_api::schema::Role::Employee => pos_app::domain::value_objects::Role::Employee,
        pos_api::schema::Role::Admin => pos_app::domain::value_objects::Role::Admin,
        pos_api::schema::Role::Owner => pos_app::domain::value_objects::Role::Owner,
        pos_api::schema::Role::SuperAdmin => pos_app::domain::value_objects::Role::SuperAdmin,
    }
}

const fn vat_mode_to_api(m: pos_app::domain::entities::VatMode) -> VatMode {
    match m {
        pos_app::domain::entities::VatMode::None => VatMode::None,
        pos_app::domain::entities::VatMode::Exclusive => VatMode::Exclusive,
        pos_app::domain::entities::VatMode::Inclusive => VatMode::Inclusive,
    }
}

const fn currency_to_api(c: pos_app::domain::entities::Currency) -> Currency {
    match c {
        pos_app::domain::entities::Currency::Thb => Currency::Thb,
        pos_app::domain::entities::Currency::Usdt => Currency::Usdt,
    }
}

const fn chain_to_api(c: pos_app::domain::value_objects::Chain) -> pos_api::schema::Chain {
    match c {
        pos_app::domain::value_objects::Chain::Solana => pos_api::schema::Chain::Solana,
        pos_app::domain::value_objects::Chain::Eth => pos_api::schema::Chain::Eth,
        pos_app::domain::value_objects::Chain::Btc => pos_api::schema::Chain::Btc,
        pos_app::domain::value_objects::Chain::Binance => pos_api::schema::Chain::Binance,
        pos_app::domain::value_objects::Chain::Bank => pos_api::schema::Chain::Bank,
    }
}

const fn chain_to_domain(c: pos_api::schema::Chain) -> pos_app::domain::value_objects::Chain {
    match c {
        pos_api::schema::Chain::Solana => pos_app::domain::value_objects::Chain::Solana,
        pos_api::schema::Chain::Eth => pos_app::domain::value_objects::Chain::Eth,
        pos_api::schema::Chain::Btc => pos_app::domain::value_objects::Chain::Btc,
        pos_api::schema::Chain::Binance => pos_app::domain::value_objects::Chain::Binance,
        pos_api::schema::Chain::Bank => pos_app::domain::value_objects::Chain::Bank,
    }
}

const fn token_to_api(t: pos_app::domain::value_objects::SplToken) -> pos_api::schema::SplToken {
    match t {
        pos_app::domain::value_objects::SplToken::Sol => pos_api::schema::SplToken::Sol,
        pos_app::domain::value_objects::SplToken::Usdc => pos_api::schema::SplToken::Usdc,
        pos_app::domain::value_objects::SplToken::Usdt => pos_api::schema::SplToken::Usdt,
    }
}

const fn token_to_domain(t: pos_api::schema::SplToken) -> pos_app::domain::value_objects::SplToken {
    match t {
        pos_api::schema::SplToken::Sol => pos_app::domain::value_objects::SplToken::Sol,
        pos_api::schema::SplToken::Usdc => pos_app::domain::value_objects::SplToken::Usdc,
        pos_api::schema::SplToken::Usdt => pos_app::domain::value_objects::SplToken::Usdt,
    }
}

pub fn build_router(state: AppServices) -> Router {
    let router = Router::new()
        .route(routes::HEALTH, get(health))
        .route(routes::AUTH_NONCE, post(handlers::auth_nonce))
        .route(routes::AUTH_VERIFY, post(handlers::auth_verify))
        .route(routes::AUTH_LOGOUT, post(handlers::auth_logout))
        .route(routes::AUTH_ME, get(handlers::auth_me))
        .route(routes::PAYMENTS_ORDERS, post(handlers::create_order))
        .route(routes::PAYMENTS_VERIFY, post(handlers::verify_payment))
        .route(
            routes::SUBSCRIPTIONS_CURRENT,
            get(handlers::subscription_current),
        )
        .route(routes::SUBSCRIPTIONS_RENEW, post(handlers::renew))
        .route(
            routes::SHOPS,
            post(handlers::create_shop).get(handlers::list_shops),
        )
        .route(
            routes::SHOP_GOODS,
            post(handlers::add_goods).get(handlers::list_goods),
        )
        .route(routes::SHOP_GOODS_SALES, get(handlers::goods_sales))
        .route(
            routes::SHOP_DESTINATIONS,
            post(handlers::add_destination).get(handlers::list_destinations),
        )
        .route(
            "/shops/{shop_id}/destinations/{destination_id}",
            axum::routing::delete(handlers::remove_destination),
        )
        .route(routes::SHOP_INVITES, post(handlers::invite))
        .route(routes::SHOP_MEMBERS, get(handlers::list_members))
        .route(
            routes::SHOP_MEMBER_ROLE,
            axum::routing::put(handlers::set_member_role),
        )
        .route(routes::SHOP_CHECKOUT, post(handlers::checkout))
        .route(
            routes::SHOP_SETTINGS,
            axum::routing::put(handlers::update_settings),
        )
        .route(routes::SHOP_BILLINGS, get(handlers::list_invoices))
        .route(routes::INVITE_ACCEPT, post(handlers::accept_invite));

    #[cfg(not(target_arch = "wasm32"))]
    let router = router.route(routes::SHOP_BILLING_PDF, get(handlers::invoice_pdf));

    #[cfg(not(target_arch = "wasm32"))]
    let router = router.layer(TraceLayer::new_for_http());

    router.layer(CorsLayer::permissive()).with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

fn map_shop(e: ShopError) -> ApiErrorReponse {
    let api = match e {
        ShopError::NotMember => ApiError::Unauthorized,
        ShopError::Forbidden | ShopError::Rule(_) => ApiError::Forbidden,
        ShopError::InviteeUnknown | ShopError::NotFound => ApiError::NotFound,
        ShopError::InviteNotAcceptable => ApiError::Invalid("invite not acceptable".into()),
        ShopError::Subscription(sub_err) => map_sub(sub_err),
        ShopError::Storage(_) => ApiError::Internal,
    };
    ApiErrorReponse(api)
}

fn map_sub(e: sub_svc::SubError) -> ApiError {
    use pos_api::error::ApiError as E;
    match e {
        sub_svc::SubError::NoSubscription => {
            E::Forbidden // needs an active $1/mo subscription
        }
        sub_svc::SubError::Expired => E::Forbidden, // expired; renew to continue
        sub_svc::SubError::QuotaShops => {
            E::Invalid("shop limit reached for this subscription".into())
        }
        sub_svc::SubError::QuotaGoods => E::Invalid("goods limit reached (30 per shop)".into()),
        sub_svc::SubError::QuotaStorage => {
            E::Invalid("storage quota exceeded (10 MB per shop)".into())
        }
        sub_svc::SubError::OrderNotFound => E::NotFound,
        sub_svc::SubError::OrderAlreadyPaid => E::Conflict("order already paid".into()),
        sub_svc::SubError::Verify(err) => E::Invalid(format!("payment verification failed: {err}")),
        sub_svc::SubError::Storage(_) => E::Internal,
    }
}

mod handlers {
    use super::*;

    pub async fn auth_nonce(
        State(svc): State<AppServices>,
        Json(req): Json<AuthNonceRequest>,
    ) -> Api<AuthNonceResponse> {
        let (nonce, message, expires_at) = pos_app::service::auth::nonce(&svc, &req.wallet)
            .await
            .map_err(map_auth)?;
        Ok(axum::Json(AuthNonceResponse {
            nonce,
            message,
            expires_at,
        }))
    }

    pub async fn auth_verify(
        State(svc): State<AppServices>,
        Json(req): Json<AuthVerifyRequest>,
    ) -> Api<AuthResponse> {
        let verifier = svc.wallet_verifier.clone();
        let tokens = svc.tokens.clone();
        let (user_id, token) = pos_app::service::auth::verify(
            &svc,
            verifier.as_ref(),
            tokens.as_ref(),
            &req.wallet,
            &req.nonce,
            &req.signature,
        )
        .await
        .map_err(map_auth)?;
        Ok(axum::Json(AuthResponse {
            token,
            user_id,
            wallet: req.wallet,
        }))
    }

    /// Stateless bearer tokens: logout is client-side token discard.
    pub async fn auth_logout() -> Api<serde_json::Value> {
        Ok(axum::Json(serde_json::json!({ "logged_out": true })))
    }

    pub async fn auth_me(State(svc): State<AppServices>, auth: AuthUser) -> Api<MeResponse> {
        let user = svc
            .users
            .find_by_id(auth.user_id)
            .await
            .map_err(|_| ApiErrorReponse(ApiError::Internal))?
            .ok_or(ApiErrorReponse(ApiError::Unauthorized))?;
        let subscription = subscription_payload(&svc, auth.user_id)
            .await
            .map_err(|_| ApiErrorReponse(ApiError::Internal))?;
        Ok(axum::Json(MeResponse {
            user_id: user.id,
            wallet: user.wallet.as_ref().map(Wallet::as_str).map(str::to_owned),
            email: user
                .email
                .as_ref()
                .map(pos_app::domain::value_objects::Email::as_str)
                .map(str::to_owned),
            role: role_to_api(user.role),
            subscription,
        }))
    }

    pub async fn create_order(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Json(_req): Json<CreatePaymentOrderRequest>,
    ) -> Api<PaymentOrderResponse> {
        // renewal is just a new order + verify
        let order = sub_svc::create_order(&svc, auth.user_id)
            .await
            .map_err(|e| ApiErrorReponse(map_sub(e)))?;
        Ok(axum::Json(order_response(&order)))
    }

    pub async fn renew(
        State(svc): State<AppServices>,
        auth: AuthUser,
    ) -> Api<PaymentOrderResponse> {
        let order = sub_svc::create_order(&svc, auth.user_id)
            .await
            .map_err(|e| ApiErrorReponse(map_sub(e)))?;
        Ok(axum::Json(order_response(&order)))
    }

    pub async fn verify_payment(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Json(req): Json<VerifyPaymentRequest>,
    ) -> Api<SubscriptionResponse> {
        let verifier = svc.payment_verifier.clone();
        sub_svc::verify_order(
            &svc,
            verifier.as_ref(),
            auth.user_id,
            req.order_id,
            &req.signature,
        )
        .await
        .map_err(|e| ApiErrorReponse(map_sub(e)))?;
        subscription_payload(&svc, auth.user_id)
            .await
            .map_err(|_| ApiErrorReponse(ApiError::Internal))?
            .map(axum::Json)
            .ok_or(ApiErrorReponse(ApiError::Internal))
    }

    pub async fn subscription_current(
        State(svc): State<AppServices>,
        auth: AuthUser,
    ) -> Api<SubscriptionResponse> {
        subscription_payload(&svc, auth.user_id)
            .await
            .map_err(|_| ApiErrorReponse(ApiError::Internal))?
            .map(axum::Json)
            .ok_or(ApiErrorReponse(ApiError::NotFound))
    }

    fn order_response(
        o: &pos_app::domain::entities::subscription::PaymentOrder,
    ) -> PaymentOrderResponse {
        PaymentOrderResponse {
            order_id: o.id,
            amount_usdc: o.amount,
            mint: o.mint.clone(),
            recipient: o.recipient.clone(),
            status: match o.status {
                pos_app::domain::entities::subscription::PaymentOrderStatus::Pending => {
                    PaymentOrderStatusApi::Pending
                }
                pos_app::domain::entities::subscription::PaymentOrderStatus::Paid => {
                    PaymentOrderStatusApi::Paid
                }
            },
            created_at: o.created_at,
        }
    }

    /// Subscription + quota usage snapshot; `None` when never subscribed.
    async fn subscription_payload(
        svc: &AppServices,
        user_id: Uuid,
    ) -> Result<Option<SubscriptionResponse>, ApiErrorReponse> {
        use pos_app::domain::entities::subscription::SubscriptionStatus;
        let Some(sub) = svc.subscriptions.find_by_user(user_id).await.map_err(|e| {
            tracing::warn!("subscription_payload find_by_user: {e}");
            ApiErrorReponse(ApiError::Internal)
        })?
        else {
            return Ok(None);
        };
        let shops_used = svc
            .shops
            .find_for_user(user_id)
            .await
            .map_err(|e| {
                tracing::warn!("subscription_payload shops: {e}");
                ApiErrorReponse(ApiError::Internal)
            })?
            .len();
        let mut goods_used = 0usize;
        let mut storage_bytes = 0i64;
        for s in svc.shops.find_for_user(user_id).await.map_err(|e| {
            tracing::warn!("subscription_payload shops2: {e}");
            ApiErrorReponse(ApiError::Internal)
        })? {
            goods_used += svc
                .goods
                .find_by_shop(s.id)
                .await
                .map_err(|e| {
                    tracing::warn!("subscription_payload goods: {e}");
                    ApiErrorReponse(ApiError::Internal)
                })?
                .len();
            storage_bytes += svc.storage.total(s.id).await.map_err(|e| match e {
                pos_app::port::database::StorageError::QuotaExceeded => {
                    ApiErrorReponse(ApiError::Internal)
                }
                pos_app::port::database::StorageError::Storage(e) => {
                    tracing::warn!("subscription_payload storage: {e}");
                    ApiErrorReponse(ApiError::Internal)
                }
            })?;
        }
        Ok(Some(SubscriptionResponse {
            status: match sub.status {
                SubscriptionStatus::Active => SubscriptionStatusApi::Active,
                SubscriptionStatus::Grace => SubscriptionStatusApi::Grace,
                SubscriptionStatus::Lapsed => SubscriptionStatusApi::Lapsed,
                SubscriptionStatus::Deleted => SubscriptionStatusApi::Deleted,
            },
            started_at: Some(sub.started_at),
            expires_at: Some(sub.expires_at),
            grace_until: sub.grace_until,
            price_usdc: SUB_PRICE_USDC,
            shops_used,
            shops_quota: MAX_SHOPS_PER_USER,
            goods_used,
            goods_quota: MAX_GOODS_PER_SHOP,
            storage_bytes,
            storage_quota_bytes: MAX_STORAGE_BYTES,
        }))
    }

    fn map_auth(e: pos_app::service::auth::AuthError) -> ApiErrorReponse {
        match e {
            pos_app::service::auth::AuthError::Invalid(m) => ApiErrorReponse(ApiError::Invalid(m)),
            pos_app::service::auth::AuthError::Storage(_) => ApiErrorReponse(ApiError::Internal),
        }
    }

    pub async fn create_shop(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Json(req): Json<CreateShopRequest>,
    ) -> Api<ShopResponse> {
        let s = shop::create_shop(&svc, auth.user_id, &req.name)
            .await
            .map_err(map_shop)?;
        Ok(axum::Json(ShopResponse {
            shop_id: s.id,
            name: s.name,
            vat_mode: vat_mode_to_api(s.vat_mode),
            vat_rate: s.vat_rate,
            fx_thb_per_sol: s.fx_thb_per_sol,
            currency: currency_to_api(s.currency),
        }))
    }

    pub async fn update_settings(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Path(shop_id): Path<Uuid>,
        Json(req): Json<UpdateShopSettingsRequest>,
    ) -> Api<serde_json::Value> {
        let mode = match req.vat_mode {
            VatMode::None => pos_app::domain::entities::VatMode::None,
            VatMode::Exclusive => pos_app::domain::entities::VatMode::Exclusive,
            VatMode::Inclusive => pos_app::domain::entities::VatMode::Inclusive,
        };
        shop::update_settings(
            &svc,
            auth.user_id,
            shop_id,
            mode,
            req.vat_rate,
            req.fx_thb_per_sol,
            match req.currency {
                Currency::Thb => pos_app::domain::entities::Currency::Thb,
                Currency::Usdt => pos_app::domain::entities::Currency::Usdt,
            },
        )
        .await
        .map_err(map_shop)?;
        Ok(axum::Json(serde_json::json!({ "updated": shop_id })))
    }

    pub async fn list_shops(
        State(svc): State<AppServices>,
        auth: AuthUser,
    ) -> Api<Vec<ShopResponse>> {
        let shops = svc
            .shops
            .find_for_user(auth.user_id)
            .await
            .map_err(|_e| ApiErrorReponse::from(ApiError::Internal))?
            .into_iter()
            .map(|s| ShopResponse {
                shop_id: s.id,
                name: s.name,
                vat_mode: vat_mode_to_api(s.vat_mode),
                vat_rate: s.vat_rate,
                fx_thb_per_sol: s.fx_thb_per_sol,
                currency: currency_to_api(s.currency),
            })
            .collect();
        Ok(axum::Json(shops))
    }

    pub async fn add_goods(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Path(shop_id): Path<Uuid>,
        Json(req): Json<AddGoodsRequest>,
    ) -> Api<GoodsResponse> {
        let price = Money::new(req.price).map_err(|e| ApiError::Invalid(e.to_string()))?;
        if req.units < 0 {
            return Err(ApiErrorReponse(ApiError::Invalid(String::from(
                "units must be >= 0",
            ))));
        }
        let barcode = req
            .barcode
            .as_deref()
            .map(pos_barcode::format::normalize)
            .filter(|b| !b.is_empty());
        if let Some(b) = &barcode {
            pos_barcode::format::validate(b)
                .map_err(|e| ApiErrorReponse(ApiError::Invalid(e.to_string())))?;
        }
        let p = goods_svc::add_goods(
            &svc,
            auth.user_id,
            shop_id,
            goods_svc::NewGoods {
                name: &req.name,
                price,
                cost: req.cost,
                units: req.units,
                barcode,
                icon: pos_app::domain::value_objects::GoodsIcon::parse(&req.icon)
                    .unwrap_or_default(),
                color: pos_app::domain::value_objects::PastelColor::parse(&req.color)
                    .unwrap_or_default(),
                goods_type: pos_app::domain::value_objects::GoodsType::parse(&req.goods_type)
                    .unwrap_or_default(),
            },
        )
        .await
        .map_err(map_shop)?;
        Ok(axum::Json(goods_response(&p)))
    }

    pub async fn list_goods(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Path(shop_id): Path<Uuid>,
    ) -> Api<Vec<GoodsResponse>> {
        let items = goods_svc::list_goods(&svc, auth.user_id, shop_id)
            .await
            .map_err(map_shop)?;
        Ok(axum::Json(items.iter().map(goods_response).collect()))
    }

    fn goods_response(p: &pos_app::domain::entities::Goods) -> GoodsResponse {
        GoodsResponse {
            goods_id: p.id,
            shop_id: p.shop_id,
            name: p.name.clone(),
            price: p.price.amount().normalize(),
            cost: p.cost.normalize(),
            units: p.units,
            barcode: p.barcode.clone(),
            icon: p.icon.as_str().to_owned(),
            color: p.color.as_str().to_owned(),
            goods_type: p.goods_type.as_str().to_owned(),
        }
    }

    pub async fn goods_sales(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Path(shop_id): Path<Uuid>,
    ) -> Api<Vec<GoodsSalesResponse>> {
        let sales = goods_svc::goods_sales(&svc, auth.user_id, shop_id)
            .await
            .map_err(map_shop)?;
        Ok(axum::Json(
            sales
                .into_iter()
                .map(|(goods_id, sold)| GoodsSalesResponse { goods_id, sold })
                .collect(),
        ))
    }

    pub async fn add_destination(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Path(shop_id): Path<Uuid>,
        Json(req): Json<AddDestinationRequest>,
    ) -> Api<DestinationResponse> {
        let address = ChainAddress::parse(chain_to_domain(req.chain), &req.address)
            .map_err(|e| ApiError::Invalid(e.to_string()))?;
        let d = dest_svc::add_destination(
            &svc,
            auth.user_id,
            shop_id,
            address,
            token_to_domain(req.token),
        )
        .await
        .map_err(map_shop)?;
        Ok(axum::Json(destination_response(&d)))
    }

    pub async fn list_destinations(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Path(shop_id): Path<Uuid>,
    ) -> Api<Vec<DestinationResponse>> {
        let items = dest_svc::list_destinations(&svc, auth.user_id, shop_id)
            .await
            .map_err(map_shop)?;
        Ok(axum::Json(items.iter().map(destination_response).collect()))
    }

    pub async fn remove_destination(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Path((shop_id, destination_id)): Path<(Uuid, Uuid)>,
    ) -> Api<serde_json::Value> {
        dest_svc::remove_destination(&svc, auth.user_id, shop_id, destination_id)
            .await
            .map_err(map_shop)?;
        Ok(axum::Json(serde_json::json!({ "removed": destination_id })))
    }

    pub async fn invite(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Path(shop_id): Path<Uuid>,
        Json(req): Json<InviteRequest>,
    ) -> Api<InviteResponse> {
        let email = Email::parse(&req.email).map_err(|e| ApiError::Invalid(e.to_string()))?;
        let invite = shop::invite(
            &svc,
            auth.user_id,
            shop_id,
            email.as_str(),
            role_to_domain(req.role),
        )
        .await
        .map_err(map_shop)?;
        Ok(axum::Json(InviteResponse {
            invite_id: invite.id,
            shop_id: invite.shop_id,
            email: invite.email.as_str().to_owned(),
            role: role_to_api(invite.role),
        }))
    }

    pub async fn accept_invite(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Path(invite_id): Path<Uuid>,
        Json(_req): Json<InviteAcceptRequest>,
    ) -> Api<ShopMemberResponse> {
        let user = svc
            .users
            .find_by_id(auth.user_id)
            .await
            .map_err(|_| ApiErrorReponse(ApiError::Internal))?
            .ok_or(ApiErrorReponse(ApiError::Unauthorized))?;
        let member = shop::accept_invite(&svc, &user, invite_id)
            .await
            .map_err(map_shop)?;
        let label = svc
            .users
            .find_by_id(member.user_id)
            .await
            .ok()
            .flatten()
            .map(|u| u.label().to_owned())
            .unwrap_or_default();
        Ok(axum::Json(ShopMemberResponse {
            user_id: member.user_id,
            email: label,
            role: role_to_api(member.role),
        }))
    }

    pub async fn list_members(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Path(shop_id): Path<Uuid>,
    ) -> Api<Vec<ShopMemberResponse>> {
        shop::require_min_role(
            &svc,
            auth.user_id,
            shop_id,
            pos_app::domain::value_objects::Role::Admin,
        )
        .await
        .map_err(map_shop)?;
        let members = shop::list_members(&svc, shop_id).await.map_err(map_shop)?;
        Ok(axum::Json(
            members
                .into_iter()
                .map(|(m, email)| ShopMemberResponse {
                    user_id: m.user_id,
                    email: email.unwrap_or_default(),
                    role: role_to_api(m.role),
                })
                .collect(),
        ))
    }

    pub async fn set_member_role(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Path((shop_id, user_id)): Path<(Uuid, Uuid)>,
        Json(req): Json<UpdateMemberRoleRequest>,
    ) -> Api<serde_json::Value> {
        shop::set_member_role(
            &svc,
            auth.user_id,
            shop_id,
            user_id,
            role_to_domain(req.role),
        )
        .await
        .map_err(map_shop)?;
        Ok(axum::Json(serde_json::json!({ "updated": user_id })))
    }

    pub async fn checkout(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Path(shop_id): Path<Uuid>,
        Json(req): Json<CheckoutRequest>,
    ) -> Api<pos_api::schema::InvoiceResponse> {
        shop::require_min_role(
            &svc,
            auth.user_id,
            shop_id,
            pos_app::domain::value_objects::Role::Employee,
        )
        .await
        .map_err(map_shop)?;
        let destination = svc
            .destinations
            .find_by_id(req.destination_id)
            .await
            .map_err(|_| ApiErrorReponse(ApiError::Internal))?
            .ok_or(ApiErrorReponse(ApiError::NotFound))?;
        if destination.shop_id != shop_id {
            return Err(ApiErrorReponse(ApiError::NotFound));
        }
        let shop = svc
            .shops
            .find_by_id(shop_id)
            .await
            .map_err(|_| ApiErrorReponse(ApiError::Internal))?
            .ok_or(ApiErrorReponse(ApiError::NotFound))?;
        let qr = svc.qr.clone();
        let (invoice, _qr_png) = pos_app::service::invoice::checkout(
            &svc,
            qr.as_ref(),
            svc.promptpay.as_ref(),
            &shop,
            &req.goods_ids,
            destination,
        )
        .await
        .map_err(|e| match e {
            pos_app::service::invoice::InvoiceError::NoGoods => {
                ApiError::Invalid("no goods selected".into())
            }
            pos_app::service::invoice::InvoiceError::Storage(ref msg) => {
                tracing::debug!("checkout storage: {msg}");
                ApiError::Internal
            }
        })?;
        let payload = pos_app::service::invoice::pay_payload(
            svc.promptpay.as_ref(),
            &invoice.destination,
            pos_app::service::invoice::qr_amount(
                invoice.total(),
                invoice.destination.address.chain(),
                shop.fx_thb_per_sol,
            ),
        );
        Ok(axum::Json(invoice_response(
            &invoice,
            &payload,
            pos_app::service::invoice::qr_amount(
                invoice.total(),
                invoice.destination.address.chain(),
                shop.fx_thb_per_sol,
            ),
        )))
    }

    pub async fn list_invoices(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Path(shop_id): Path<Uuid>,
    ) -> Api<Vec<pos_api::schema::InvoiceResponse>> {
        shop::require_min_role(
            &svc,
            auth.user_id,
            shop_id,
            pos_app::domain::value_objects::Role::Employee,
        )
        .await
        .map_err(map_shop)?;
        let invoices = svc
            .invoices
            .list_by_shop(shop_id)
            .await
            .map_err(|_| ApiErrorReponse(ApiError::Internal))?;
        let fx = svc
            .shops
            .find_by_id(shop_id)
            .await
            .map_err(|_| ApiErrorReponse(ApiError::Internal))?
            .map(|s| s.fx_thb_per_sol)
            .unwrap_or(pos_api::schema::DECIMAL_ZERO);
        let mut out = Vec::with_capacity(invoices.len());
        for inv in invoices {
            let amount = pos_app::service::invoice::qr_amount(
                inv.total(),
                inv.destination.address.chain(),
                fx,
            );
            let payload = pos_app::service::invoice::pay_payload(
                svc.promptpay.as_ref(),
                &inv.destination,
                amount,
            );
            out.push(invoice_response(&inv, &payload, amount));
        }
        Ok(axum::Json(out))
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn invoice_pdf(
        State(svc): State<AppServices>,
        auth: AuthUser,
        Path((shop_id, invoice_id)): Path<(Uuid, Uuid)>,
    ) -> Result<axum::response::Response, ApiErrorReponse> {
        shop::require_min_role(
            &svc,
            auth.user_id,
            shop_id,
            pos_app::domain::value_objects::Role::Employee,
        )
        .await
        .map_err(map_shop)?;
        let invoice = svc
            .invoices
            .find_by_id(invoice_id)
            .await
            .map_err(|_| ApiErrorReponse(ApiError::Internal))?
            .ok_or(ApiErrorReponse(ApiError::NotFound))?;
        if invoice.shop_id != shop_id {
            return Err(ApiErrorReponse(ApiError::NotFound));
        }
        let shop = svc
            .shops
            .find_by_id(shop_id)
            .await
            .map_err(|_| ApiErrorReponse(ApiError::Internal))?
            .ok_or(ApiErrorReponse(ApiError::NotFound))?;
        let payload = pos_app::service::invoice::pay_payload(
            svc.promptpay.as_ref(),
            &invoice.destination,
            pos_app::service::invoice::qr_amount(
                invoice.total(),
                invoice.destination.address.chain(),
                shop.fx_thb_per_sol,
            ),
        );
        let qr_png = svc
            .qr
            .render_png(&payload)
            .await
            .map_err(|_| ApiErrorReponse(ApiError::Internal))?;
        let bytes = pos_app::service::pdf::render_invoice_pdf(
            &invoice,
            &shop.name,
            &qr_png,
            shop.currency,
            shop.fx_thb_per_sol,
        )
        .map_err(|_| ApiErrorReponse(ApiError::Internal))?;
        // Billing PDFs count toward the 10 MB shop quota (once per invoice).
        sub_svc::record_storage(&svc, shop_id, invoice_id, bytes.len() as i64)
            .await
            .map_err(|e| ApiErrorReponse(map_sub(e)))?;
        axum::response::Response::builder()
            .header("content-type", "application/pdf")
            .header(
                "content-disposition",
                format!("attachment; filename=\"billing-{invoice_id}.pdf\""),
            )
            .body(axum::body::Body::from(bytes))
            .map_err(|_| ApiErrorReponse(ApiError::Internal))
    }

    fn destination_response(
        d: &pos_app::domain::entities::PaymentDestination,
    ) -> DestinationResponse {
        DestinationResponse {
            destination_id: d.id,
            shop_id: d.shop_id,
            chain: chain_to_api(d.address.chain()),
            address: d.address.as_str().to_owned(),
            token: token_to_api(d.token),
            label: None,
        }
    }

    fn invoice_response(
        inv: &pos_app::domain::entities::Invoice,
        payload: &str,
        qr_amount: rust_decimal::Decimal,
    ) -> pos_api::schema::InvoiceResponse {
        let lines: Vec<pos_api::schema::InvoiceLineResponse> = inv
            .lines
            .iter()
            .map(|l| pos_api::schema::InvoiceLineResponse {
                name: l.name.clone(),
                price: l.price.amount().normalize(),
                qty: l.qty,
                cost: l.cost.normalize(),
            })
            .collect();
        pos_api::schema::InvoiceResponse {
            invoice_id: inv.id,
            shop_id: inv.shop_id,
            total: inv.total().normalize(),
            qr_amount,
            subtotal: inv.subtotal().normalize(),
            vat_mode: vat_mode_to_api(inv.vat_mode),
            vat_rate: inv.vat_rate.normalize(),
            vat_amount: inv.vat_amount().normalize(),
            cost: inv.cost().normalize(),
            lines,
            destination: DestinationResponse {
                destination_id: inv.destination.id,
                shop_id: inv.destination.shop_id,
                chain: chain_to_api(inv.destination.address.chain()),
                address: inv.destination.address.as_str().to_owned(),
                token: token_to_api(inv.destination.token),
                label: None,
            },
            qr_payload: payload.to_owned(),
            pdf_url: format!("/shops/{}/billings/{}/pdf", inv.shop_id, inv.id),
        }
    }
}
