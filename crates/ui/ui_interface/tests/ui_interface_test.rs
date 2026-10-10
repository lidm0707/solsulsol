//! Trait tests with a scripted in-memory client.

use pos_api::schema::{
    AddGoodsRequest, AuthNonceRequest, AuthNonceResponse, AuthResponse, AuthVerifyRequest,
    CheckoutRequest, CreatePaymentOrderRequest, CreateShopRequest, GoodsResponse, InviteRequest,
    InviteResponse, MeResponse, PaymentOrderResponse, Role, ShopResponse, SubscriptionResponse,
    SubscriptionStatusApi, UpdateShopSettingsRequest, VatMode, VerifyPaymentRequest,
};
use ui_interface::traits::{ApiClient, InviteFilter, TokenStore};
use uuid::Uuid;

struct Scripted {
    pub token: String,
    pub nonce: String,
    pub goods: std::sync::Mutex<Vec<GoodsResponse>>,
}

fn sample_subscription() -> SubscriptionResponse {
    SubscriptionResponse {
        status: SubscriptionStatusApi::Active,
        started_at: None,
        expires_at: None,
        grace_until: None,
        price_usdc: rust_decimal::Decimal::ONE,
        shops_used: 0,
        shops_quota: 1,
        goods_used: 0,
        goods_quota: 30,
        storage_bytes: 0,
        storage_quota_bytes: 10 * 1024 * 1024,
    }
}

impl ApiClient for Scripted {
    async fn auth_nonce(&self, req: &AuthNonceRequest) -> Result<AuthNonceResponse, String> {
        Ok(AuthNonceResponse {
            nonce: self.nonce.clone(),
            message: format!("sign-in: {}", req.wallet),
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
        })
    }
    async fn auth_verify(&self, req: &AuthVerifyRequest) -> Result<AuthResponse, String> {
        if req.nonce != self.nonce || req.signature.is_empty() {
            return Err(String::from("bad nonce or signature"));
        }
        Ok(AuthResponse {
            token: self.token.clone(),
            user_id: Uuid::now_v7(),
            wallet: req.wallet.clone(),
        })
    }
    async fn auth_logout(&self, _token: &str) -> Result<(), String> {
        Ok(())
    }
    async fn auth_me(&self, _token: &str) -> Result<MeResponse, String> {
        Err(String::from("not scripted"))
    }
    async fn create_payment_order(
        &self,
        _token: &str,
        _req: &CreatePaymentOrderRequest,
    ) -> Result<PaymentOrderResponse, String> {
        Ok(PaymentOrderResponse {
            order_id: Uuid::now_v7(),
            amount_usdc: rust_decimal::Decimal::ONE,
            mint: String::from("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
            recipient: String::from("7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU"),
            status: pos_api::schema::PaymentOrderStatusApi::Pending,
            created_at: chrono::Utc::now(),
        })
    }
    async fn verify_payment(
        &self,
        _token: &str,
        req: &VerifyPaymentRequest,
    ) -> Result<SubscriptionResponse, String> {
        if req.signature.is_empty() {
            return Err(String::from("payment not found"));
        }
        Ok(sample_subscription())
    }
    async fn subscription_current(&self, _token: &str) -> Result<SubscriptionResponse, String> {
        Ok(sample_subscription())
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
            fx_thb_per_sol: rust_decimal::Decimal::ZERO,
            currency: Default::default(),
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
            icon: req.icon.clone(),
            color: req.color.clone(),
            goods_type: req.goods_type.clone(),
        };
        if let Ok(mut v) = self.goods.lock() {
            v.push(g.clone());
        }
        Ok(g)
    }
    async fn list_goods(&self, _t: &str, _s: Uuid) -> Result<Vec<GoodsResponse>, String> {
        Ok(self.goods.lock().map(|v| v.clone()).unwrap_or_default())
    }
    async fn list_goods_sales(
        &self,
        _t: &str,
        _s: Uuid,
    ) -> Result<Vec<pos_api::schema::GoodsSalesResponse>, String> {
        Ok(Vec::new())
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
async fn wallet_signin_stores_token_and_goods_round_trip() {
    let client = Scripted {
        token: String::from("t0k3n"),
        nonce: String::from("n0nc3"),
        goods: std::sync::Mutex::new(Vec::new()),
    };
    let store = MemStore::default();
    let wallet = String::from("7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU");

    let nonce = client
        .auth_nonce(&AuthNonceRequest {
            wallet: wallet.clone(),
        })
        .await
        .expect("nonce ok");
    let res = client
        .auth_verify(&AuthVerifyRequest {
            wallet: wallet.clone(),
            nonce: nonce.nonce,
            signature: String::from("5ign4tur3"),
        })
        .await
        .expect("verify ok");
    store.save(&res.token).await;
    assert_eq!(store.load().await.as_deref(), Some("t0k3n"));

    let bad = client
        .auth_verify(&AuthVerifyRequest {
            wallet: wallet.clone(),
            nonce: String::from("wrong"),
            signature: String::from("5ign4tur3"),
        })
        .await;
    assert!(bad.is_err());

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
                icon: String::new(),
                color: String::new(),
                goods_type: String::from("other"),
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
    client.auth_logout("t0k3n").await.expect("logout ok");
    store.clear().await;
    assert_eq!(store.load().await, None);
}

#[tokio::test]
async fn payment_order_verifies_into_subscription() {
    let client = Scripted {
        token: String::from("t"),
        nonce: String::from("n"),
        goods: std::sync::Mutex::new(Vec::new()),
    };
    let order = client
        .create_payment_order("t", &CreatePaymentOrderRequest { renew: false })
        .await
        .expect("order ok");
    assert_eq!(order.amount_usdc, rust_decimal::Decimal::ONE);
    assert!(!order.recipient.is_empty());

    let bad = client
        .verify_payment(
            "t",
            &VerifyPaymentRequest {
                order_id: order.order_id,
                signature: String::new(),
            },
        )
        .await;
    assert!(bad.is_err());

    let sub = client
        .verify_payment(
            "t",
            &VerifyPaymentRequest {
                order_id: order.order_id,
                signature: String::from("5ol"),
            },
        )
        .await
        .expect("verify ok");
    assert_eq!(sub.status, SubscriptionStatusApi::Active);
    assert_eq!(sub.shops_quota, 1);
    assert_eq!(sub.goods_quota, 30);
    assert_eq!(sub.storage_quota_bytes, 10 * 1024 * 1024);

    let cur = client.subscription_current("t").await.expect("current ok");
    assert_eq!(cur.status, SubscriptionStatusApi::Active);
}

#[tokio::test]
async fn invite_preserves_role() {
    let client = Scripted {
        token: String::from("t"),
        nonce: String::from("n"),
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
