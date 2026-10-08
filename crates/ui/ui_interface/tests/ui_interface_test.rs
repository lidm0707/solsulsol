//! Trait tests with a scripted in-memory client.

use pos_api::schema::{
    AddGoodsRequest, CheckoutRequest, CreateShopRequest, GoodsResponse, InviteRequest,
    InviteResponse, LoginRequest, LoginResponse, RegisterRequest, Role, ShopResponse,
    UpdateShopSettingsRequest, VatMode,
};
use ui_interface::traits::{ApiClient, InviteFilter, TokenStore};
use uuid::Uuid;

struct Scripted {
    pub token: String,
    pub goods: std::sync::Mutex<Vec<GoodsResponse>>,
}

impl ApiClient for Scripted {
    async fn register(&self, _req: &RegisterRequest) -> Result<Uuid, String> {
        Ok(Uuid::now_v7())
    }
    async fn login(&self, _req: &LoginRequest) -> Result<LoginResponse, String> {
        Ok(LoginResponse {
            token: self.token.clone(),
            user_id: Uuid::now_v7(),
        })
    }
    async fn create_shop(
        &self,
        _token: &str,
        req: &CreateShopRequest,
    ) -> Result<ShopResponse, String> {
        Ok(ShopResponse {
            shop_id: Uuid::now_v7(),
            name: req.name.clone(),
            vat_mode: VatMode::None,
            vat_rate: rust_decimal::Decimal::ZERO,
        })
    }
    async fn list_shops(&self, _token: &str) -> Result<Vec<ShopResponse>, String> {
        Ok(Vec::new())
    }
    async fn update_shop_settings(
        &self,
        _token: &str,
        _shop_id: Uuid,
        _req: &UpdateShopSettingsRequest,
    ) -> Result<(), String> {
        Ok(())
    }
    async fn invite(&self, _t: &str, req: &InviteRequest) -> Result<InviteResponse, String> {
        Ok(InviteResponse {
            invite_id: Uuid::now_v7(),
            shop_id: req.shop_id,
            email: req.email.clone(),
            role: req.role,
        })
    }
    async fn accept_invite(
        &self,
        _t: &str,
        _i: Uuid,
        _r: &pos_api::schema::InviteAcceptRequest,
    ) -> Result<pos_api::schema::ShopMemberResponse, String> {
        Err(String::from("not scripted"))
    }
    async fn list_members(
        &self,
        _t: &str,
        _s: Uuid,
    ) -> Result<Vec<pos_api::schema::ShopMemberResponse>, String> {
        Ok(Vec::new())
    }
    async fn set_member_role(
        &self,
        _t: &str,
        _s: Uuid,
        _u: Uuid,
        _r: &pos_api::schema::UpdateMemberRoleRequest,
    ) -> Result<(), String> {
        Ok(())
    }
    async fn add_goods(&self, _t: &str, req: &AddGoodsRequest) -> Result<GoodsResponse, String> {
        let g = GoodsResponse {
            goods_id: Uuid::now_v7(),
            shop_id: req.shop_id,
            name: req.name.clone(),
            price: req.price,
            cost: req.cost,
            units: req.units,
            barcode: req.barcode.clone(),
        };
        if let Ok(mut v) = self.goods.lock() {
            v.push(g.clone());
        }
        Ok(g)
    }
    async fn list_goods(&self, _t: &str, _s: Uuid) -> Result<Vec<GoodsResponse>, String> {
        Ok(self.goods.lock().map(|v| v.clone()).unwrap_or_default())
    }
    async fn add_destination(
        &self,
        _t: &str,
        _r: &pos_api::schema::AddDestinationRequest,
    ) -> Result<pos_api::schema::DestinationResponse, String> {
        Err(String::from("not scripted"))
    }
    async fn list_destinations(
        &self,
        _t: &str,
        _s: Uuid,
    ) -> Result<Vec<pos_api::schema::DestinationResponse>, String> {
        Ok(Vec::new())
    }
    async fn checkout(
        &self,
        _t: &str,
        _r: &CheckoutRequest,
    ) -> Result<pos_api::schema::InvoiceResponse, String> {
        Err(String::from("not scripted"))
    }
    async fn list_invoices(
        &self,
        _t: &str,
        _s: Uuid,
    ) -> Result<Vec<pos_api::schema::InvoiceResponse>, String> {
        Ok(Vec::new())
    }
}

#[derive(Default)]
struct MemStore(std::sync::Mutex<Option<String>>);

impl TokenStore for MemStore {
    async fn save(&self, token: &str) {
        if let Ok(mut v) = self.0.lock() {
            *v = Some(token.to_owned());
        }
    }
    async fn load(&self) -> Option<String> {
        self.0.lock().ok().and_then(|v| v.clone())
    }
    async fn clear(&self) {
        if let Ok(mut v) = self.0.lock() {
            *v = None;
        }
    }
}

#[tokio::test]
async fn login_stores_token_and_goods_round_trip() {
    let client = Scripted {
        token: String::from("t0k3n"),
        goods: std::sync::Mutex::new(Vec::new()),
    };
    let store = MemStore::default();

    let res = client
        .login(&LoginRequest {
            email: String::from("a@b.com"),
            password: String::from("hunter2234"),
        })
        .await
        .expect("login ok");
    store.save(&res.token).await;
    assert_eq!(store.load().await.as_deref(), Some("t0k3n"));

    let shop_id = client
        .create_shop(
            "t0k3n",
            &CreateShopRequest {
                name: String::from("Cafe"),
            },
        )
        .await
        .expect("shop ok")
        .shop_id;
    client
        .add_goods(
            "t0k3n",
            &AddGoodsRequest {
                shop_id,
                name: String::from("tea"),
                price: rust_decimal::Decimal::from(3),
                cost: rust_decimal::Decimal::from(1),
                units: 5,
                barcode: None,
            },
        )
        .await
        .expect("goods ok");
    assert_eq!(
        client
            .list_goods("t0k3n", shop_id)
            .await
            .expect("list ok")
            .len(),
        1
    );
    store.clear().await;
    assert_eq!(store.load().await, None);
}

#[tokio::test]
async fn invite_preserves_role() {
    let client = Scripted {
        token: String::from("t"),
        goods: std::sync::Mutex::new(Vec::new()),
    };
    let inv = client
        .invite(
            "t",
            &InviteRequest {
                shop_id: Uuid::now_v7(),
                email: String::from("x@y.dev"),
                role: Role::Admin,
            },
        )
        .await
        .expect("invite ok");
    assert_eq!(inv.role, Role::Admin);
}

#[test]
fn invite_filter_gates_super_admin_rows() {
    assert!(InviteFilter::All.keeps(Role::SuperAdmin));
    assert!(!InviteFilter::Pending.keeps(Role::SuperAdmin));
    assert!(InviteFilter::Pending.keeps(Role::Admin));
}
