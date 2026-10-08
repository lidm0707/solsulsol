//! Routes. State is `AppServices` directly (Clone).

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use pos_api::error::ApiError;
use pos_api::routes;
use pos_api::schema::{
    AddDestinationRequest, AddGoodsRequest, CheckoutRequest, CreateShopRequest,
    DestinationResponse, GoodsResponse, InviteAcceptRequest, InviteRequest, InviteResponse,
    LoginRequest, LoginResponse, RegisterRequest, ShopMemberResponse, ShopResponse,
    UpdateMemberRoleRequest, UpdateShopSettingsRequest, VatMode,
};
use tower_http::cors::CorsLayer;
#[cfg(not(target_arch = "wasm32"))]
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::error::ApiErrorReponse;
use crate::extract::AuthUser;
use pos_app::domain::value_objects::{ChainAddress, Email, Money};
use pos_app::service::destination as dest_svc;
use pos_app::service::goods as goods_svc;
use pos_app::service::shop::{self, ShopError};
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
        .route(routes::REGISTER, post(handlers::register))
        .route(routes::LOGIN, post(handlers::login))
        .route(
            routes::SHOPS,
            post(handlers::create_shop).get(handlers::list_shops),
        )
        .route(
            routes::SHOP_GOODS,
            post(handlers::add_goods).get(handlers::list_goods),
        )
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
        ShopError::Storage(_) => ApiError::Internal,
    };
    ApiErrorReponse(api)
}

mod handlers {
    use super::*;

    pub async fn register(
        State(svc): State<AppServices>,
        Json(req): Json<RegisterRequest>,
    ) -> Api<serde_json::Value> {
        let hasher = svc.hasher.clone();
        let user =
            pos_app::service::auth::register(&svc, hasher.as_ref(), &req.email, &req.password)
                .await
                .map_err(|e| match e {
                    pos_app::service::auth::AuthError::EmailTaken => {
                        ApiError::Conflict("email taken".into())
                    }
                    pos_app::service::auth::AuthError::BadCredentials => ApiError::Unauthorized,
                    pos_app::service::auth::AuthError::Invalid(e) => ApiError::Invalid(e),
                    pos_app::service::auth::AuthError::Storage(_) => ApiError::Internal,
                })?;
        Ok(axum::Json(serde_json::json!({ "user_id": user.id })))
    }

    pub async fn login(
        State(svc): State<AppServices>,
        Json(req): Json<LoginRequest>,
    ) -> Api<LoginResponse> {
        let hasher = svc.hasher.clone();
        let tokens = svc.tokens.clone();
        let (user_id, token) = pos_app::service::auth::login(
            &svc,
            hasher.as_ref(),
            tokens.as_ref(),
            &req.email,
            &req.password,
        )
        .await
        .map_err(|e| match e {
            pos_app::service::auth::AuthError::EmailTaken => {
                ApiError::Conflict("email taken".into())
            }
            pos_app::service::auth::AuthError::BadCredentials => ApiError::Unauthorized,
            pos_app::service::auth::AuthError::Invalid(e) => ApiError::Invalid(e),
            pos_app::service::auth::AuthError::Storage(_) => ApiError::Internal,
        })?;
        Ok(axum::Json(LoginResponse { token, user_id }))
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
        shop::update_settings(&svc, auth.user_id, shop_id, mode, req.vat_rate)
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

    fn goods_response(p: &pos_app::domain::entities::Product) -> GoodsResponse {
        GoodsResponse {
            goods_id: p.id,
            shop_id: p.shop_id,
            name: p.name.clone(),
            price: p.price.amount().normalize(),
            cost: p.cost.normalize(),
            units: p.units,
            barcode: p.barcode.clone(),
        }
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
        let email = svc
            .users
            .find_by_id(member.user_id)
            .await
            .ok()
            .flatten()
            .map(|u| u.email.as_str().to_owned())
            .unwrap_or_default();
        Ok(axum::Json(ShopMemberResponse {
            user_id: member.user_id,
            email,
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
        let (invoice, qr_png) = pos_app::service::invoice::checkout(
            &svc,
            qr.as_ref(),
            svc.promptpay.as_ref(),
            &shop,
            &req.goods_ids,
            destination,
        )
        .await
        .map_err(|e| match e {
            pos_app::service::invoice::InvoiceError::NoProducts => {
                ApiError::Invalid("no products selected".into())
            }
            pos_app::service::invoice::InvoiceError::Storage(ref msg) => {
                tracing::debug!("checkout storage: {msg}");
                ApiError::Internal
            }
        })?;
        let payload = pos_app::service::invoice::pay_payload(
            svc.promptpay.as_ref(),
            &invoice.destination,
            invoice.total(),
        );
        Ok(axum::Json(invoice_response(
            &invoice,
            &payload,
            qr_png.len(),
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
        let mut out = Vec::with_capacity(invoices.len());
        for inv in invoices {
            let payload = pos_app::service::invoice::pay_payload(
                svc.promptpay.as_ref(),
                &inv.destination,
                inv.total(),
            );
            out.push(invoice_response(&inv, &payload, 0));
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
            invoice.total(),
        );
        let qr_png = svc
            .qr
            .render_png(&payload)
            .await
            .map_err(|_| ApiErrorReponse(ApiError::Internal))?;
        let bytes = pos_app::service::pdf::render_invoice_pdf(&invoice, &shop.name, &qr_png)
            .map_err(|_| ApiErrorReponse(ApiError::Internal))?;
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
        qr_bytes: usize,
    ) -> pos_api::schema::InvoiceResponse {
        let _ = qr_bytes;
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
