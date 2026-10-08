//! HTTP ApiClient for wasm via gloo-net fetch.

use gloo_net::http::{Method, Request, RequestBuilder};
use pos_api::schema::*;
use ui_interface::traits::ApiClient;
use uuid::Uuid;

pub struct FetchClient {
    pub base_url: String,
}

const JSON: &str = "application/json";

impl FetchClient {
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    async fn json<T: serde::de::DeserializeOwned>(req: Request) -> Result<T, String> {
        let res = req.send().await.map_err(|e| e.to_string())?;
        if !res.ok() {
            return Err(format!("http {}", res.status()));
        }
        res.json::<T>().await.map_err(|e| e.to_string())
    }

    async fn empty(req: Request) -> Result<(), String> {
        let res = req.send().await.map_err(|e| e.to_string())?;
        if res.ok() {
            Ok(())
        } else {
            Err(format!("http {}", res.status()))
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
    async fn register(&self, req: &RegisterRequest) -> Result<Uuid, String> {
        #[derive(serde::Deserialize)]
        struct Out {
            user_id: Uuid,
        }
        let req = self.post_json(pos_api::routes::REGISTER, None, req)?;
        let out: Out = Self::json(req).await?;
        Ok(out.user_id)
    }

    async fn login(&self, req: &LoginRequest) -> Result<LoginResponse, String> {
        let req = self.post_json(pos_api::routes::LOGIN, None, req)?;
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
    ) -> Result<InvoiceResponse, String> {
        let path = pos_api::routes::SHOP_CHECKOUT.replace("{shop_id}", &req.shop_id.to_string());
        let req = self.post_json(&path, Some(token), req)?;
        Self::json(req).await
    }

    async fn list_invoices(
        &self,
        token: &str,
        shop_id: Uuid,
    ) -> Result<Vec<InvoiceResponse>, String> {
        let path = pos_api::routes::SHOP_BILLINGS.replace("{shop_id}", &shop_id.to_string());
        let req = self.get_authed(&path, token);
        Self::json(req).await
    }
}
