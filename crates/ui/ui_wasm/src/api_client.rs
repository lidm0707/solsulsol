//! HTTP ApiClient for wasm via gloo-net fetch.

use gloo_net::http::{Method, Request, RequestBuilder};
use pos_api::schema::*;
use ui_interface::traits::ApiClient;
use uuid::Uuid;

pub struct FetchClient {
    pub base_url: String,
}

const JSON: &str = "application/json";
const HTTP_FORBIDDEN: u16 = 403;
const SUBSCRIPTION_REQUIRED: &str = "Active subscription required — open the Subscription tab";

#[derive(serde::Serialize)]
struct EmptyBody;

impl FetchClient {
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    async fn json<T: serde::de::DeserializeOwned>(req: Request) -> Result<T, String> {
        let res = req.send().await.map_err(|e| e.to_string())?;
        if !res.ok() {
            return Err(err_msg(res.status(), res.text().await.ok().as_deref()));
        }
        res.json::<T>().await.map_err(|e| e.to_string())
    }

    async fn empty(req: Request) -> Result<(), String> {
        let res = req.send().await.map_err(|e| e.to_string())?;
        if res.ok() {
            Ok(())
        } else {
            Err(err_msg(res.status(), res.text().await.ok().as_deref()))
        }
    }

    fn serialize(body: &impl serde::Serialize) -> Result<String, String> {
        serde_json::to_string(body).map_err(|e| e.to_string())
    }

    fn post_json(
        &self,
        path: &str,
        token: Option<&str>,
        body: &impl serde::Serialize,
    ) -> Result<Request, String> {
        let mut b = RequestBuilder::new(&self.url(path))
            .method(Method::POST)
            .header("Content-Type", JSON);
        if let Some(t) = token {
            b = b.header("Authorization", &format!("Bearer {t}"));
        }
        b.body(Self::serialize(body)?).map_err(|e| e.to_string())
    }

    fn get_authed(&self, path: &str, token: &str) -> Request {
        RequestBuilder::new(&self.url(path))
            .header("Authorization", &format!("Bearer {token}"))
            .build()
            .unwrap_or_else(|e| impossible_request(&e.to_string()))
    }

    fn delete_authed(&self, path: &str, token: &str) -> Request {
        RequestBuilder::new(&self.url(path))
            .method(Method::DELETE)
            .header("Authorization", &format!("Bearer {token}"))
            .build()
            .unwrap_or_else(|e| impossible_request(&e.to_string()))
    }

    fn put_json(
        &self,
        path: &str,
        token: &str,
        body: &impl serde::Serialize,
    ) -> Result<Request, String> {
        RequestBuilder::new(&self.url(path))
            .method(Method::PUT)
            .header("Content-Type", JSON)
            .header("Authorization", &format!("Bearer {token}"))
            .body(Self::serialize(body)?)
            .map_err(|e| e.to_string())
    }
}

#[derive(serde::Deserialize)]
struct ErrorBody {
    error: String,
}

fn err_msg(status: u16, body: Option<&str>) -> String {
    if status == HTTP_FORBIDDEN {
        return String::from(SUBSCRIPTION_REQUIRED);
    }
    if let Some(text) = body
        && let Ok(parsed) = serde_json::from_str::<ErrorBody>(text)
    {
        return parsed.error;
    }
    format!("http {status}")
}

// A headerless GET cannot fail to build; this only silences the type system.
fn impossible_request(e: &str) -> Request {
    web_console(&format!("unreachable request build failure: {e}"));
    unreachable_via_option().expect("headerless GET always builds")
}

fn unreachable_via_option() -> Option<Request> {
    RequestBuilder::new("about:invalid").build().ok()
}

fn web_console(msg: &str) {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&msg.to_owned().into());
    #[cfg(not(target_arch = "wasm32"))]
    let _ = msg;
}

impl ApiClient for FetchClient {
    async fn auth_nonce(&self, req: &AuthNonceRequest) -> Result<AuthNonceResponse, String> {
        let req = self.post_json(pos_api::routes::AUTH_NONCE, None, req)?;
        Self::json(req).await
    }

    async fn auth_verify(&self, req: &AuthVerifyRequest) -> Result<AuthResponse, String> {
        let req = self.post_json(pos_api::routes::AUTH_VERIFY, None, req)?;
        Self::json(req).await
    }

    async fn auth_logout(&self, token: &str) -> Result<(), String> {
        let req = self.post_json(pos_api::routes::AUTH_LOGOUT, Some(token), &EmptyBody)?;
        Self::empty(req).await
    }

    async fn auth_me(&self, token: &str) -> Result<MeResponse, String> {
        let req = self.get_authed(pos_api::routes::AUTH_ME, token);
        Self::json(req).await
    }

    async fn create_payment_order(
        &self,
        token: &str,
        req: &CreatePaymentOrderRequest,
    ) -> Result<PaymentOrderResponse, String> {
        let req = self.post_json(pos_api::routes::PAYMENTS_ORDERS, Some(token), req)?;
        Self::json(req).await
    }

    async fn verify_payment(
        &self,
        token: &str,
        req: &VerifyPaymentRequest,
    ) -> Result<SubscriptionResponse, String> {
        let req = self.post_json(pos_api::routes::PAYMENTS_VERIFY, Some(token), req)?;
        Self::json(req).await
    }

    async fn subscription_current(&self, token: &str) -> Result<SubscriptionResponse, String> {
        let req = self.get_authed(pos_api::routes::SUBSCRIPTIONS_CURRENT, token);
        Self::json(req).await
    }

    async fn create_shop(
        &self,
        token: &str,
        req: &CreateShopRequest,
    ) -> Result<ShopResponse, String> {
        let req = self.post_json(pos_api::routes::SHOPS, Some(token), req)?;
        Self::json(req).await
    }

    async fn list_shops(&self, token: &str) -> Result<Vec<ShopResponse>, String> {
        let req = self.get_authed(pos_api::routes::SHOPS, token);
        Self::json(req).await
    }

    async fn update_shop_settings(
        &self,
        token: &str,
        shop_id: Uuid,
        req: &UpdateShopSettingsRequest,
    ) -> Result<(), String> {
        let path = pos_api::routes::SHOP_SETTINGS.replace("{shop_id}", &shop_id.to_string());
        let req = self.put_json(&path, token, req)?;
        Self::empty(req).await
    }

    async fn invite(&self, token: &str, req: &InviteRequest) -> Result<InviteResponse, String> {
        let path = pos_api::routes::SHOP_INVITES.replace("{shop_id}", &req.shop_id.to_string());
        let req = self.post_json(&path, Some(token), req)?;
        Self::json(req).await
    }

    async fn accept_invite(
        &self,
        token: &str,
        invite_id: Uuid,
        req: &InviteAcceptRequest,
    ) -> Result<ShopMemberResponse, String> {
        let path = pos_api::routes::INVITE_ACCEPT.replace("{invite_id}", &invite_id.to_string());
        let req = self.post_json(&path, Some(token), req)?;
        Self::json(req).await
    }

    async fn list_members(
        &self,
        token: &str,
        shop_id: Uuid,
    ) -> Result<Vec<ShopMemberResponse>, String> {
        let path = pos_api::routes::SHOP_MEMBERS.replace("{shop_id}", &shop_id.to_string());
        let req = self.get_authed(&path, token);
        Self::json(req).await
    }

    async fn set_member_role(
        &self,
        token: &str,
        shop_id: Uuid,
        user_id: Uuid,
        req: &UpdateMemberRoleRequest,
    ) -> Result<(), String> {
        let path = pos_api::routes::SHOP_MEMBER_ROLE
            .replace("{shop_id}", &shop_id.to_string())
            .replace("{user_id}", &user_id.to_string());
        let req = self.put_json(&path, token, req)?;
        Self::empty(req).await
    }

    async fn add_goods(&self, token: &str, req: &AddGoodsRequest) -> Result<GoodsResponse, String> {
        let path = pos_api::routes::SHOP_GOODS.replace("{shop_id}", &req.shop_id.to_string());
        let req = self.post_json(&path, Some(token), req)?;
        Self::json(req).await
    }

    async fn list_goods(&self, token: &str, shop_id: Uuid) -> Result<Vec<GoodsResponse>, String> {
        let path = pos_api::routes::SHOP_GOODS.replace("{shop_id}", &shop_id.to_string());
        let req = self.get_authed(&path, token);
        Self::json(req).await
    }

    async fn list_goods_sales(
        &self,
        token: &str,
        shop_id: Uuid,
    ) -> Result<Vec<GoodsSalesResponse>, String> {
        let path = pos_api::routes::SHOP_GOODS_SALES.replace("{shop_id}", &shop_id.to_string());
        let req = self.get_authed(&path, token);
        Self::json(req).await
    }

    async fn add_destination(
        &self,
        token: &str,
        req: &AddDestinationRequest,
    ) -> Result<DestinationResponse, String> {
        let path =
            pos_api::routes::SHOP_DESTINATIONS.replace("{shop_id}", &req.shop_id.to_string());
        let req = self.post_json(&path, Some(token), req)?;
        Self::json(req).await
    }

    async fn list_destinations(
        &self,
        token: &str,
        shop_id: Uuid,
    ) -> Result<Vec<DestinationResponse>, String> {
        let path = pos_api::routes::SHOP_DESTINATIONS.replace("{shop_id}", &shop_id.to_string());
        let req = self.get_authed(&path, token);
        Self::json(req).await
    }

    async fn checkout(
        &self,
        token: &str,
        req: &CheckoutRequest,
    ) -> Result<BillingResponse, String> {
        let path = pos_api::routes::SHOP_CHECKOUT.replace("{shop_id}", &req.shop_id.to_string());
        let req = self.post_json(&path, Some(token), req)?;
        Self::json(req).await
    }

    async fn list_billings(
        &self,
        token: &str,
        shop_id: Uuid,
    ) -> Result<Vec<BillingResponse>, String> {
        let path = pos_api::routes::SHOP_BILLINGS.replace("{shop_id}", &shop_id.to_string());
        let req = self.get_authed(&path, token);
        Self::json(req).await
    }

    async fn delete_goods(&self, token: &str, shop_id: Uuid, goods_id: Uuid) -> Result<(), String> {
        let path = pos_api::routes::SHOP_GOODS_ID
            .replace("{shop_id}", &shop_id.to_string())
            .replace("{goods_id}", &goods_id.to_string());
        let req = self.delete_authed(&path, token);
        Self::empty(req).await
    }

    async fn list_storage_objects(
        &self,
        token: &str,
        shop_id: Uuid,
    ) -> Result<Vec<StorageObjectResponse>, String> {
        let path = pos_api::routes::SHOP_STORAGE_OBJECTS.replace("{shop_id}", &shop_id.to_string());
        let req = self.get_authed(&path, token);
        Self::json(req).await
    }

    async fn delete_storage_object(
        &self,
        token: &str,
        shop_id: Uuid,
        key: Uuid,
    ) -> Result<(), String> {
        let path = pos_api::routes::SHOP_STORAGE_OBJECT
            .replace("{shop_id}", &shop_id.to_string())
            .replace("{key}", &key.to_string());
        let req = self.delete_authed(&path, token);
        Self::empty(req).await
    }
}
