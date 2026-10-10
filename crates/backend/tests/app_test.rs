//! App-layer use-case tests (in-memory fakes from `common`).
#![allow(clippy::todo, clippy::unimplemented)]

mod common;

use common::{
    RECIPIENT, ShopMemberRepo, ShopRepo, UserRepo, grant_active, services, test_config, wallet_user,
};
use pos_backend::app::service::shop::{self, ShopError};
use pos_backend::app::service::subscription::SubError;
use pos_backend::domain::value_objects::{Chain, ChainAddress, Role, SplToken};
use rust_decimal::Decimal;
use uuid::Uuid;

const SOL: &str = "4Nd1mBQtrMJVYDfZS3vNs3p8zQH4wVzLB6s5aXKc9PqR";
const SHOP: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000001");

#[tokio::test]
async fn admin_can_invite_employee_but_not_owner() {
    let admin_id = Uuid::now_v7();
    let (svc, w) = services(test_config(RECIPIENT));
    w.users.insert(&wallet_user(admin_id)).await.unwrap();
    let member = pos_backend::domain::entities::ShopMember {
        shop_id: SHOP,
        user_id: admin_id,
        role: Role::Admin,
    };
    w.shops
        .insert(&pos_backend::domain::entities::Shop {
            id: SHOP,
            owner_id: admin_id,
            name: "t".into(),
            vat_mode: Default::default(),
            vat_rate: Decimal::ZERO,
            fx_thb_per_sol: Decimal::ZERO,
            currency: Default::default(),
            deleted_at: None,
        })
        .await
        .unwrap();
    w.members.insert(&member).await.unwrap();

    let invite = shop::invite(&svc, admin_id, SHOP, "e@x.com", Role::Employee)
        .await
        .expect("admin invites employee");
    assert_eq!(
        invite.status,
        pos_backend::domain::entities::InviteStatus::Pending
    );

    assert!(matches!(
        shop::invite(&svc, admin_id, SHOP, "e@x.com", Role::Owner).await,
        Err(ShopError::Rule(_))
    ));
}

#[tokio::test]
async fn destination_requires_admin() {
    let emp_id = Uuid::now_v7();
    let (svc, w) = services(test_config(RECIPIENT));
    w.users.insert(&wallet_user(emp_id)).await.unwrap();
    w.members
        .insert(&pos_backend::domain::entities::ShopMember {
            shop_id: SHOP,
            user_id: emp_id,
            role: Role::Employee,
        })
        .await
        .unwrap();
    let addr = ChainAddress::parse(Chain::Solana, SOL).expect("test sol address is valid");
    assert!(matches!(
        pos_backend::app::service::destination::add_destination(
            &svc,
            emp_id,
            SHOP,
            addr,
            SplToken::Sol
        )
        .await,
        Err(ShopError::Forbidden)
    ));
}

#[tokio::test]
async fn goods_round_trip_with_admin_role() {
    let admin_id = Uuid::now_v7();
    let (svc, w) = services(test_config(RECIPIENT));
    w.users.insert(&wallet_user(admin_id)).await.unwrap();
    grant_active(&svc, admin_id).await;
    w.members
        .insert(&pos_backend::domain::entities::ShopMember {
            shop_id: SHOP,
            user_id: admin_id,
            role: Role::Admin,
        })
        .await
        .unwrap();

    let money = pos_backend::domain::value_objects::Money::new(Decimal::from(5))
        .expect("test amount is valid");
    let p = pos_backend::app::service::goods::add_goods(
        &svc,
        admin_id,
        SHOP,
        pos_backend::app::service::goods::NewGoods {
            name: "coffee",
            price: money,
            cost: Decimal::from(2),
            units: 10,
            barcode: Some(String::from("4006381333931")),
            icon: Default::default(),
            color: Default::default(),
            goods_type: Default::default(),
        },
    )
    .await
    .expect("admin adds goods");
    assert_eq!(p.name, "coffee");
    assert_eq!(p.barcode.as_deref(), Some("4006381333931"));

    let listed = pos_backend::app::service::goods::list_goods(&svc, admin_id, SHOP)
        .await
        .expect("lists goods");
    assert_eq!(listed.len(), 1);
}

#[tokio::test]
async fn goods_rejected_without_active_subscription() {
    let admin_id = Uuid::now_v7();
    let (svc, w) = services(test_config(RECIPIENT));
    w.users.insert(&wallet_user(admin_id)).await.unwrap();
    w.members
        .insert(&pos_backend::domain::entities::ShopMember {
            shop_id: SHOP,
            user_id: admin_id,
            role: Role::Admin,
        })
        .await
        .unwrap();
    let money = pos_backend::domain::value_objects::Money::new(Decimal::from(5))
        .expect("test amount is valid");
    assert!(matches!(
        pos_backend::app::service::goods::add_goods(
            &svc,
            admin_id,
            SHOP,
            pos_backend::app::service::goods::NewGoods {
                name: "x",
                price: money,
                cost: Decimal::ZERO,
                units: 1,
                barcode: None,
                icon: Default::default(),
                color: Default::default(),
            goods_type: Default::default(),
            },
        )
        .await,
        Err(ShopError::Subscription(SubError::NoSubscription))
    ));
}
