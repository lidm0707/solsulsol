//! D1 repositories implementing the pos-port storage traits.
//!
//! D1 (SQLite) has no interactive transactions, so `BillingRepo::insert`
//! writes the billing header + lines as one atomic `batch()`. Decimals and
//! timestamps are stored as TEXT and parsed back on read.

use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pos_app::domain::entities::subscription::{
    AuthNonce, PaymentOrder, PaymentOrderStatus, Subscription, SubscriptionStatus,
};
use pos_app::domain::entities::{
    Billing, BillingLine, BillingStatus, Currency, Goods, Invite, InviteStatus, PaymentDestination,
    Shop, ShopMember, User, VatMode,
};
use pos_app::domain::value_objects::{
    Chain, ChainAddress, Email, GoodsIcon, GoodsType, Money, PastelColor, Role, SplToken, Wallet,
};
use pos_app::port::database::{
    BillingRepo, DestinationRepo, GoodsRepo, InviteRepo, NonceRepo, PaymentOrderRepo,
    ShopMemberRepo, ShopRepo, StorageError, StorageUsageRepo, SubscriptionRepo, UserRepo,
};

use rust_decimal::Decimal;
use uuid::Uuid;
use worker::console_error;
use worker::wasm_bindgen::JsValue;
use worker::D1Database;

pub const NIL: Uuid = Uuid::nil();
const FALLBACK_ADDRESS: &str = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";

pub type Db = Arc<D1Database>;

// --- JS value helpers ---

fn js_text(v: &str) -> JsValue {
    JsValue::from_str(v)
}

fn js_uid(v: Uuid) -> JsValue {
    JsValue::from_str(&v.to_string())
}

fn js_dec(v: Decimal) -> JsValue {
    JsValue::from_str(&v.to_string())
}

fn js_ts(v: DateTime<Utc>) -> JsValue {
    JsValue::from_str(&v.to_rfc3339())
}

fn js_i64(v: i64) -> JsValue {
    JsValue::from_f64(v as f64)
}

fn js_opt_text(v: Option<&str>) -> JsValue {
    match v {
        Some(s) => JsValue::from_str(s),
        None => JsValue::NULL,
    }
}

fn err(e: worker::Error) -> String {
    console_error!("d1: {e}");
    e.to_string()
}

async fn query_one<T>(db: &Db, sql: &str, vals: &[JsValue]) -> Result<Option<T>, String>
where
    T: for<'de> serde::de::Deserialize<'de>,
{
    let stmt = db.prepare(sql).bind(vals).map_err(err)?;
    worker::send::SendFuture::new(stmt.first::<T>(None))
        .await
        .map_err(err)
}

async fn query_all<T>(db: &Db, sql: &str, vals: &[JsValue]) -> Result<Vec<T>, String>
where
    T: for<'de> serde::de::Deserialize<'de>,
{
    let stmt = db.prepare(sql).bind(vals).map_err(err)?;
    let res = worker::send::SendFuture::new(stmt.all())
        .await
        .map_err(err)?;
    res.results::<T>().map_err(err)
}

async fn exec(db: &Db, sql: &str, vals: &[JsValue]) -> Result<(), String> {
    let stmt = db.prepare(sql).bind(vals).map_err(err)?;
    let res = worker::send::SendFuture::new(stmt.run())
        .await
        .map_err(err)?;
    match res.error() {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

async fn exec_count(db: &Db, sql: &str, vals: &[JsValue]) -> Result<u64, String> {
    let stmt = db.prepare(sql).bind(vals).map_err(err)?;
    let res = worker::send::SendFuture::new(stmt.run())
        .await
        .map_err(err)?;
    match res.error() {
        Some(e) => Err(e),
        None => {
            let changed = res
                .meta()
                .map_err(err)?
                .and_then(|m| m.changes)
                .unwrap_or(0);
            Ok(changed as u64)
        }
    }
}

// --- row parsing ---

fn parse_uid(s: &str) -> Uuid {
    Uuid::parse_str(s).unwrap_or(NIL)
}

fn parse_ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn parse_dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap_or(Decimal::ZERO)
}

/// D1 NUMERIC columns come back as JS numbers when affinity converts them;
/// accept both strings and numbers for decimal/text-stored-as-number fields.
fn num_str<'de, D>(de: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct NumStr;
    impl serde::de::Visitor<'_> for NumStr {
        type Value = String;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("string or number")
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<String, E> {
            Ok(v.to_owned())
        }
        fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<String, E> {
            Ok(v.to_string())
        }
        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<String, E> {
            Ok(v.to_string())
        }
        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<String, E> {
            Ok(v.to_string())
        }
    }
    de.deserialize_any(NumStr)
}

#[derive(serde::Deserialize)]
struct UserRow {
    id: String,
    email: Option<String>,
    wallet: Option<String>,
    role: String,
    created_at: String,
}

fn user_row(r: UserRow) -> User {
    User {
        id: parse_uid(&r.id),
        email: r
            .email
            .map(|e| Email::parse(&e).unwrap_or_else(|_| Email::unchecked(e))),
        wallet: r
            .wallet
            .map(|w| Wallet::parse(&w).unwrap_or_else(|_| Wallet::unchecked(w))),
        role: Role::parse(&r.role).unwrap_or(Role::Employee),
        created_at: parse_ts(&r.created_at),
    }
}

#[derive(serde::Deserialize)]
struct ShopRow {
    id: String,
    owner_id: String,
    name: String,
    vat_mode: String,
    #[serde(deserialize_with = "num_str")]
    vat_rate: String,
    #[serde(default, deserialize_with = "num_str")]
    fx_thb_per_usd: String,
    #[serde(default)]
    currency: String,
    #[serde(default)]
    deleted_at: Option<String>,
}

fn shop_row(r: ShopRow) -> Shop {
    Shop {
        id: parse_uid(&r.id),
        owner_id: parse_uid(&r.owner_id),
        name: r.name,
        vat_mode: VatMode::parse(&r.vat_mode).unwrap_or_default(),
        vat_rate: parse_dec(&r.vat_rate),
        fx_thb_per_usd: parse_dec(&r.fx_thb_per_usd),
        currency: Currency::parse(&r.currency).unwrap_or_default(),
        deleted_at: r.deleted_at.as_deref().map(parse_ts),
    }
}

#[derive(serde::Deserialize)]
struct MemberRow {
    shop_id: String,
    user_id: String,
    role: String,
}

#[derive(serde::Deserialize)]
struct MemberRoleRow {
    role: String,
}

fn member_row(r: MemberRow) -> ShopMember {
    ShopMember {
        shop_id: parse_uid(&r.shop_id),
        user_id: parse_uid(&r.user_id),
        role: Role::parse(&r.role).unwrap_or(Role::Employee),
    }
}

#[derive(serde::Deserialize)]
struct GoodsRow {
    id: String,
    shop_id: String,
    name: String,
    #[serde(deserialize_with = "num_str")]
    price: String,
    #[serde(deserialize_with = "num_str")]
    cost: String,
    units: i64,
    barcode: Option<String>,
    icon: Option<String>,
    color: Option<String>,
    goods_type: Option<String>,
}

fn goods_row(r: GoodsRow) -> Goods {
    let price = parse_dec(&r.price);
    Goods {
        id: parse_uid(&r.id),
        shop_id: parse_uid(&r.shop_id),
        name: r.name,
        price: Money::new(price).unwrap_or_else(|_| Money::one()),
        cost: parse_dec(&r.cost),
        units: r.units,
        barcode: r.barcode,
        icon: r
            .icon
            .as_deref()
            .and_then(GoodsIcon::parse)
            .unwrap_or_default(),
        color: r
            .color
            .as_deref()
            .and_then(PastelColor::parse)
            .unwrap_or_default(),
        goods_type: r
            .goods_type
            .as_deref()
            .and_then(GoodsType::parse)
            .unwrap_or_default(),
    }
}

#[derive(serde::Deserialize)]
struct DestinationRow {
    id: String,
    shop_id: String,
    chain: String,
    address: String,
    token: String,
}

fn destination_row(r: DestinationRow) -> PaymentDestination {
    let chain = Chain::parse(&r.chain).unwrap_or(Chain::Solana);
    PaymentDestination {
        id: parse_uid(&r.id),
        shop_id: parse_uid(&r.shop_id),
        address: ChainAddress::parse(chain, &r.address)
            .unwrap_or_else(|_| ChainAddress::unchecked(chain, r.address)),
        token: SplToken::parse(&r.token).unwrap_or_default(),
    }
}

#[derive(serde::Deserialize)]
struct InviteRow {
    id: String,
    shop_id: String,
    email: String,
    role: String,
    status: String,
    expires_at: String,
}

fn invite_row(r: InviteRow) -> Invite {
    Invite {
        id: parse_uid(&r.id),
        shop_id: parse_uid(&r.shop_id),
        email: Email::parse(&r.email).unwrap_or_else(|_| Email::unchecked(r.email)),
        role: Role::parse(&r.role).unwrap_or(Role::Employee),
        status: InviteStatus::parse(&r.status).unwrap_or(InviteStatus::Revoked),
        expires_at: parse_ts(&r.expires_at),
    }
}

#[derive(serde::Deserialize)]
struct BillingHeadRow {
    id: String,
    shop_id: String,
    status: String,
    destination_id: String,
    vat_mode: String,
    #[serde(deserialize_with = "num_str")]
    vat_rate: String,
}

fn head_row(r: BillingHeadRow) -> (Uuid, Uuid, BillingStatus, Uuid, VatMode, Decimal) {
    (
        parse_uid(&r.id),
        parse_uid(&r.shop_id),
        BillingStatus::parse(&r.status).unwrap_or(BillingStatus::Pending),
        parse_uid(&r.destination_id),
        VatMode::parse(&r.vat_mode).unwrap_or_default(),
        parse_dec(&r.vat_rate),
    )
}

#[derive(serde::Deserialize)]
struct LineRow {
    goods_id: String,
    name: String,
    #[serde(deserialize_with = "num_str")]
    price: String,
    #[serde(deserialize_with = "num_str")]
    cost: String,
    qty: i64,
}

fn line_row(r: LineRow) -> BillingLine {
    let price = parse_dec(&r.price);
    BillingLine {
        goods_id: parse_uid(&r.goods_id),
        name: r.name,
        price: Money::new(price).unwrap_or_else(|_| Money::one()),
        cost: parse_dec(&r.cost),
        qty: u32::try_from(r.qty).unwrap_or(0),
    }
}

// --- users ---

pub struct D1UserRepo {
    pub db: Db,
}

/// D1 users.email stays NOT NULL (SQLite cannot ALTER it); wallet-only
/// users get a synthetic address that can never collide with real invites.
const USER_INSERT: &str = "INSERT INTO users (id, email, wallet, role, created_at) \
 VALUES (?1, ?2, ?3, ?4, ?5)";
const USER_SELECT: &str = "SELECT id, email, wallet, role, created_at FROM users";

#[async_trait]
impl UserRepo for D1UserRepo {
    async fn insert(&self, user: &User) -> Result<(), String> {
        exec(
            &self.db,
            USER_INSERT,
            &[
                js_uid(user.id),
                js_opt_text(user.email.as_ref().map(Email::as_str)),
                js_opt_text(user.wallet.as_ref().map(Wallet::as_str)),
                js_text(user.role.as_str()),
                js_ts(user.created_at),
            ],
        )
        .await
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, String> {
        query_one(
            &self.db,
            &format!("{USER_SELECT} WHERE id = ?1"),
            &[js_uid(id)],
        )
        .await
        .map(|r: Option<UserRow>| r.map(user_row))
    }

    async fn find_by_wallet(&self, wallet: &str) -> Result<Option<User>, String> {
        query_one(
            &self.db,
            &format!("{USER_SELECT} WHERE wallet = ?1"),
            &[js_text(wallet)],
        )
        .await
        .map(|r: Option<UserRow>| r.map(user_row))
    }
}

// --- shops ---

pub struct D1ShopRepo {
    pub db: Db,
}

const SHOP_INSERT: &str =
    "INSERT INTO shops (id, owner_id, name, vat_mode, vat_rate, fx_thb_per_usd, currency, deleted_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";
const SHOP_SELECT: &str =
    "SELECT id, owner_id, name, vat_mode, vat_rate, fx_thb_per_usd, currency, deleted_at FROM shops WHERE deleted_at IS NULL";
const SHOP_SOFT_DELETE: &str =
    "UPDATE shops SET deleted_at = ?2 WHERE owner_id = ?1 AND deleted_at IS NULL";
const SHOP_HARD_DELETE: &str = "DELETE FROM shops WHERE deleted_at IS NOT NULL AND deleted_at < ?1";

#[async_trait]
impl ShopRepo for D1ShopRepo {
    async fn insert(&self, shop: &Shop) -> Result<(), String> {
        exec(
            &self.db,
            SHOP_INSERT,
            &[
                js_uid(shop.id),
                js_uid(shop.owner_id),
                js_text(&shop.name),
                js_text(shop.vat_mode.as_str()),
                js_dec(shop.vat_rate),
                js_dec(shop.fx_thb_per_usd),
                js_text(shop.currency.as_str()),
                js_opt_text(shop.deleted_at.as_ref().map(|d| d.to_rfc3339()).as_deref()),
            ],
        )
        .await
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Shop>, String> {
        query_one(
            &self.db,
            &format!("{SHOP_SELECT} AND id = ?1"),
            &[js_uid(id)],
        )
        .await
        .map(|r: Option<ShopRow>| r.map(shop_row))
    }

    async fn find_for_user(&self, user_id: Uuid) -> Result<Vec<Shop>, String> {
        query_all(
            &self.db,
            "SELECT s.id, s.owner_id, s.name, s.vat_mode, s.vat_rate, s.fx_thb_per_usd, s.currency FROM shops s \
             JOIN shop_members m ON m.shop_id = s.id WHERE m.user_id = ?1 AND s.deleted_at IS NULL",
            &[js_uid(user_id)],
        )
        .await
        .map(|rows: Vec<ShopRow>| rows.into_iter().map(shop_row).collect())
    }

    async fn update_settings(
        &self,
        id: Uuid,
        vat_mode: VatMode,
        vat_rate: Decimal,
        fx_thb_per_usd: Decimal,
        currency: Currency,
    ) -> Result<(), String> {
        exec(
            &self.db,
            "UPDATE shops SET vat_mode = ?2, vat_rate = ?3, fx_thb_per_usd = ?4, currency = ?5 WHERE id = ?1",
            &[
                js_uid(id),
                js_text(vat_mode.as_str()),
                js_dec(vat_rate),
                js_dec(fx_thb_per_usd),
                js_text(currency.as_str()),
            ],
        )
        .await
    }

    async fn soft_delete_for_owner(
        &self,
        owner_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<u64, String> {
        exec_count(&self.db, SHOP_SOFT_DELETE, &[js_uid(owner_id), js_ts(now)]).await
    }

    async fn hard_delete_expired(&self, before: DateTime<Utc>) -> Result<u64, String> {
        exec_count(&self.db, SHOP_HARD_DELETE, &[js_ts(before)]).await
    }
}

// --- shop members ---

pub struct D1MemberRepo {
    pub db: Db,
}

#[async_trait]
impl ShopMemberRepo for D1MemberRepo {
    async fn insert(&self, member: &ShopMember) -> Result<(), String> {
        exec(
            &self.db,
            "INSERT INTO shop_members (shop_id, user_id, role) VALUES (?1, ?2, ?3)",
            &[
                js_uid(member.shop_id),
                js_uid(member.user_id),
                js_text(member.role.as_str()),
            ],
        )
        .await
    }

    async fn find_role(&self, shop_id: Uuid, user_id: Uuid) -> Result<Option<Role>, String> {
        query_one(
            &self.db,
            "SELECT role FROM shop_members WHERE shop_id = ?1 AND user_id = ?2",
            &[js_uid(shop_id), js_uid(user_id)],
        )
        .await
        .map(|r: Option<MemberRoleRow>| r.and_then(|m| Role::parse(&m.role)))
    }

    async fn list_by_shop(&self, shop_id: Uuid) -> Result<Vec<ShopMember>, String> {
        query_all(
            &self.db,
            "SELECT shop_id, user_id, role FROM shop_members WHERE shop_id = ?1",
            &[js_uid(shop_id)],
        )
        .await
        .map(|rows: Vec<MemberRow>| rows.into_iter().map(member_row).collect())
    }

    async fn update_role(&self, shop_id: Uuid, user_id: Uuid, role: Role) -> Result<(), String> {
        exec(
            &self.db,
            "UPDATE shop_members SET role = ?3 WHERE shop_id = ?1 AND user_id = ?2",
            &[js_uid(shop_id), js_uid(user_id), js_text(role.as_str())],
        )
        .await
    }
}

// --- goods ---

pub struct D1GoodsRepo {
    pub db: Db,
}

const GOODS_INSERT: &str =
    "INSERT INTO goods (id, shop_id, name, price, cost, units, barcode, icon, color, goods_type) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";
const GOODS_SELECT: &str =
    "SELECT id, shop_id, name, price, cost, units, barcode, icon, color, goods_type FROM goods";

#[async_trait]
impl GoodsRepo for D1GoodsRepo {
    async fn insert(&self, goods: &Goods) -> Result<(), String> {
        exec(
            &self.db,
            GOODS_INSERT,
            &[
                js_uid(goods.id),
                js_uid(goods.shop_id),
                js_text(&goods.name),
                js_dec(goods.price.amount()),
                js_dec(goods.cost),
                js_i64(goods.units),
                match &goods.barcode {
                    Some(b) => js_text(b),
                    None => JsValue::NULL,
                },
                js_text(goods.icon.as_str()),
                js_text(goods.color.as_str()),
                js_text(goods.goods_type.as_str()),
            ],
        )
        .await
    }

    async fn find_by_shop(&self, shop_id: Uuid) -> Result<Vec<Goods>, String> {
        query_all(
            &self.db,
            &format!("{GOODS_SELECT} WHERE shop_id = ?1"),
            &[js_uid(shop_id)],
        )
        .await
        .map(|rows: Vec<GoodsRow>| rows.into_iter().map(goods_row).collect())
    }

    async fn take_units(&self, goods_id: Uuid) -> Result<(), String> {
        let stmt = self
            .db
            .prepare("UPDATE goods SET units = units - 1 WHERE id = ?1 AND units > 0")
            .bind(&[js_uid(goods_id)])
            .map_err(err)?;
        let res = worker::send::SendFuture::new(stmt.run())
            .await
            .map_err(err)?;
        let changed = res
            .meta()
            .map_err(err)?
            .and_then(|m| m.changes)
            .unwrap_or(0);
        if changed == 0 {
            return Err(String::from("goods out of stock"));
        }
        Ok(())
    }

    async fn find_by_id(&self, goods_id: Uuid) -> Result<Option<Goods>, String> {
        query_one(
            &self.db,
            &format!("{GOODS_SELECT} WHERE id = ?1"),
            &[js_uid(goods_id)],
        )
        .await
        .map(|r: Option<GoodsRow>| r.map(goods_row))
    }

    async fn update(&self, goods: &Goods) -> Result<(), String> {
        exec(
            &self.db,
            "UPDATE goods SET name = ?1, price = ?2, cost = ?3, units = ?4, barcode = ?5, icon = ?6, color = ?7, goods_type = ?8 WHERE id = ?9",
            &[
                js_text(&goods.name),
                js_dec(goods.price.amount()),
                js_dec(goods.cost),
                js_i64(goods.units),
                match &goods.barcode {
                    Some(b) => js_text(b),
                    None => JsValue::NULL,
                },
                js_text(goods.icon.as_str()),
                js_text(goods.color.as_str()),
                js_text(goods.goods_type.as_str()),
                js_uid(goods.id),
            ],
        )
        .await
    }

    async fn delete(&self, goods_id: Uuid) -> Result<(), String> {
        exec(
            &self.db,
            "DELETE FROM goods WHERE id = ?1",
            &[js_uid(goods_id)],
        )
        .await
    }
}

// --- destinations ---

pub struct D1DestinationRepo {
    pub db: Db,
}

const DESTINATION_INSERT: &str =
    "INSERT INTO destinations (id, shop_id, chain, address, token) VALUES (?1, ?2, ?3, ?4, ?5)";
const DESTINATION_SELECT: &str = "SELECT id, shop_id, chain, address, token FROM destinations";

#[async_trait]
impl DestinationRepo for D1DestinationRepo {
    async fn insert(&self, d: &PaymentDestination) -> Result<(), String> {
        exec(
            &self.db,
            DESTINATION_INSERT,
            &[
                js_uid(d.id),
                js_uid(d.shop_id),
                js_text(d.address.chain().as_str()),
                js_text(d.address.as_str()),
                js_text(d.token.as_str()),
            ],
        )
        .await
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<PaymentDestination>, String> {
        query_one(
            &self.db,
            &format!("{DESTINATION_SELECT} WHERE id = ?1"),
            &[js_uid(id)],
        )
        .await
        .map(|r: Option<DestinationRow>| r.map(destination_row))
    }

    async fn list_by_shop(&self, shop_id: Uuid) -> Result<Vec<PaymentDestination>, String> {
        query_all(
            &self.db,
            &format!("{DESTINATION_SELECT} WHERE shop_id = ?1"),
            &[js_uid(shop_id)],
        )
        .await
        .map(|rows: Vec<DestinationRow>| rows.into_iter().map(destination_row).collect())
    }

    async fn delete(&self, id: Uuid) -> Result<(), String> {
        exec(
            &self.db,
            "DELETE FROM destinations WHERE id = ?1",
            &[js_uid(id)],
        )
        .await
    }
}

// --- invites ---

pub struct D1InviteRepo {
    pub db: Db,
}

const INVITE_INSERT: &str =
    "INSERT INTO invites (id, shop_id, email, role, status, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";
const INVITE_SELECT: &str = "SELECT id, shop_id, email, role, status, expires_at FROM invites";

#[async_trait]
impl InviteRepo for D1InviteRepo {
    async fn insert(&self, invite: &Invite) -> Result<(), String> {
        exec(
            &self.db,
            INVITE_INSERT,
            &[
                js_uid(invite.id),
                js_uid(invite.shop_id),
                js_text(invite.email.as_str()),
                js_text(invite.role.as_str()),
                js_text(invite.status.as_str()),
                js_ts(invite.expires_at),
            ],
        )
        .await
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Invite>, String> {
        query_one(
            &self.db,
            &format!("{INVITE_SELECT} WHERE id = ?1"),
            &[js_uid(id)],
        )
        .await
        .map(|r: Option<InviteRow>| r.map(invite_row))
    }

    async fn list_by_shop(&self, shop_id: Uuid) -> Result<Vec<Invite>, String> {
        query_all(
            &self.db,
            &format!("{INVITE_SELECT} WHERE shop_id = ?1"),
            &[js_uid(shop_id)],
        )
        .await
        .map(|rows: Vec<InviteRow>| rows.into_iter().map(invite_row).collect())
    }

    async fn set_status(&self, id: Uuid, status: InviteStatus) -> Result<(), String> {
        exec(
            &self.db,
            "UPDATE invites SET status = ?2 WHERE id = ?1",
            &[js_uid(id), js_text(status.as_str())],
        )
        .await
    }
}

// --- billings ---

pub struct D1BillingRepo {
    pub db: Db,
}

const BILLING_INSERT: &str =
    "INSERT INTO billings (id, shop_id, status, destination_id, vat_mode, vat_rate) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";
const LINE_INSERT: &str =
    "INSERT INTO billing_lines (billing_id, goods_id, name, price, cost, qty) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";
const BILLING_SELECT: &str =
    "SELECT id, shop_id, status, destination_id, vat_mode, vat_rate FROM billings";

impl D1BillingRepo {
    async fn fetch_destination(&self, id: Uuid) -> Result<PaymentDestination, String> {
        #[derive(serde::Deserialize)]
        struct DestRefRow {
            id: String,
            chain: String,
            address: String,
            token: String,
        }
        let Some(r) = query_one::<DestRefRow>(
            &self.db,
            "SELECT id, chain, address, token FROM destinations WHERE id = ?1",
            &[js_uid(id)],
        )
        .await?
        else {
            return ChainAddress::parse(Chain::Solana, FALLBACK_ADDRESS)
                .map(|addr| PaymentDestination {
                    id: NIL,
                    shop_id: NIL,
                    address: addr,
                    token: SplToken::default(),
                })
                .map_err(|_| String::from("const fallback address must parse"));
        };
        let chain = Chain::parse(&r.chain).unwrap_or(Chain::Solana);
        Ok(PaymentDestination {
            id: parse_uid(&r.id),
            shop_id: NIL,
            address: ChainAddress::parse(chain, &r.address)
                .unwrap_or_else(|_| ChainAddress::unchecked(chain, r.address)),
            token: SplToken::parse(&r.token).unwrap_or_default(),
        })
    }

    async fn fetch_lines(&self, billing_id: Uuid) -> Result<Vec<BillingLine>, String> {
        let rows: Vec<LineRow> = query_all(
            &self.db,
            "SELECT goods_id, name, price, cost, qty FROM billing_lines WHERE billing_id = ?1",
            &[js_uid(billing_id)],
        )
        .await?;
        Ok(rows.into_iter().map(line_row).collect())
    }
}

#[async_trait]
impl BillingRepo for D1BillingRepo {
    async fn hard_delete_expired(&self, before: DateTime<Utc>) -> Result<u64, String> {
        exec_count(
            &self.db,
            "DELETE FROM billings WHERE shop_id IN (SELECT id FROM shops WHERE deleted_at IS NOT NULL AND deleted_at < ?1)",
            &[js_ts(before)],
        )
        .await
    }

    async fn insert(&self, billing: &Billing) -> Result<(), String> {
        let head = self
            .db
            .prepare(BILLING_INSERT)
            .bind(&[
                js_uid(billing.id),
                js_uid(billing.shop_id),
                js_text(billing.status.as_str()),
                js_uid(billing.destination.id),
                js_text(billing.vat_mode.as_str()),
                js_dec(billing.vat_rate),
            ])
            .map_err(err)?;
        let mut stmts = Vec::with_capacity(billing.lines.len() + 1);
        stmts.push(head);
        for l in &billing.lines {
            let stmt = self
                .db
                .prepare(LINE_INSERT)
                .bind(&[
                    js_uid(billing.id),
                    js_uid(l.goods_id),
                    js_text(&l.name),
                    js_dec(l.price.amount()),
                    js_dec(l.cost),
                    js_i64(i64::from(l.qty)),
                ])
                .map_err(err)?;
            stmts.push(stmt);
        }
        let results = worker::send::SendFuture::new(self.db.batch(stmts))
            .await
            .map_err(err)?;
        for r in &results {
            if let Some(e) = r.error() {
                return Err(e);
            }
        }
        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Billing>, String> {
        let Some(head) = query_one(
            &self.db,
            &format!("{BILLING_SELECT} WHERE id = ?1"),
            &[js_uid(id)],
        )
        .await
        .map(|r: Option<BillingHeadRow>| r.map(head_row))?
        else {
            return Ok(None);
        };
        let (id, shop_id, status, dest_id, vat_mode, vat_rate) = head;
        let destination = self.fetch_destination(dest_id).await?;
        let lines = self.fetch_lines(id).await?;
        Ok(Some(Billing {
            id,
            shop_id,
            status,
            lines,
            destination,
            vat_mode,
            vat_rate,
        }))
    }

    async fn list_by_shop(&self, shop_id: Uuid) -> Result<Vec<Billing>, String> {
        let heads: Vec<BillingHeadRow> = query_all(
            &self.db,
            &format!("{BILLING_SELECT} WHERE shop_id = ?1"),
            &[js_uid(shop_id)],
        )
        .await?;
        let mut out = Vec::with_capacity(heads.len());
        for head in heads {
            let (id, shop_id, status, dest_id, vat_mode, vat_rate) = head_row(head);
            let destination = self.fetch_destination(dest_id).await?;
            let lines = self.fetch_lines(id).await?;
            out.push(Billing {
                id,
                shop_id,
                status,
                lines,
                destination,
                vat_mode,
                vat_rate,
            });
        }
        Ok(out)
    }

    async fn sales_by_goods(&self, shop_id: Uuid) -> Result<Vec<(Uuid, u64)>, String> {
        #[derive(serde::Deserialize)]
        struct SalesRow {
            goods_id: String,
            #[serde(deserialize_with = "num_str")]
            sold: String,
        }
        let rows: Vec<SalesRow> = query_all(
            &self.db,
            "SELECT l.goods_id AS goods_id, SUM(l.qty) AS sold FROM billing_lines l \
             JOIN billings i ON i.id = l.billing_id \
             WHERE i.shop_id = ?1 AND i.status = 'paid' \
             GROUP BY l.goods_id ORDER BY SUM(l.qty) DESC",
            &[js_uid(shop_id)],
        )
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| (parse_uid(&r.goods_id), r.sold.parse::<u64>().unwrap_or(0)))
            .collect())
    }

    async fn goods_has_lines(&self, goods_id: Uuid) -> Result<bool, String> {
        #[derive(serde::Deserialize)]
        struct CountRow {
            #[serde(deserialize_with = "num_str")]
            n: String,
        }
        let row: Option<CountRow> = query_one(
            &self.db,
            "SELECT COUNT(*) AS n FROM billing_lines WHERE goods_id = ?1",
            &[js_uid(goods_id)],
        )
        .await?;
        Ok(row.is_some_and(|r| r.n.parse::<i64>().unwrap_or(0) > 0))
    }
}

// --- auth nonces ---

pub struct D1NonceRepo {
    pub db: Db,
}

const NONCE_INSERT: &str =
    "INSERT INTO auth_nonces (id, wallet, nonce, expires_at) VALUES (?1, ?2, ?3, ?4)";
const NONCE_SELECT: &str = "SELECT id, wallet, nonce, expires_at FROM auth_nonces";
const NONCE_CONSUME: &str = "UPDATE auth_nonces SET used_at = ?3 \
     WHERE wallet = ?1 AND nonce = ?2 AND used_at IS NULL AND expires_at > ?3";

#[derive(serde::Deserialize)]
struct NonceRow {
    id: String,
    wallet: String,
    nonce: String,
    expires_at: String,
}

fn nonce_row(r: NonceRow) -> AuthNonce {
    AuthNonce {
        id: parse_uid(&r.id),
        wallet: r.wallet,
        nonce: r.nonce,
        expires_at: parse_ts(&r.expires_at),
    }
}

#[async_trait]
impl NonceRepo for D1NonceRepo {
    async fn insert(&self, nonce: &AuthNonce) -> Result<(), String> {
        exec(
            &self.db,
            NONCE_INSERT,
            &[
                js_uid(nonce.id),
                js_text(&nonce.wallet),
                js_text(&nonce.nonce),
                js_ts(nonce.expires_at),
            ],
        )
        .await
    }

    async fn find(&self, wallet: &str, nonce: &str) -> Result<Option<AuthNonce>, String> {
        query_one(
            &self.db,
            &format!("{NONCE_SELECT} WHERE wallet = ?1 AND nonce = ?2"),
            &[js_text(wallet), js_text(nonce)],
        )
        .await
        .map(|r: Option<NonceRow>| r.map(nonce_row))
    }

    async fn consume(&self, wallet: &str, nonce: &str, now: DateTime<Utc>) -> Result<bool, String> {
        let changed = exec_count(
            &self.db,
            NONCE_CONSUME,
            &[js_text(wallet), js_text(nonce), js_ts(now)],
        )
        .await?;
        Ok(changed == 1)
    }
}

// --- subscriptions ---

pub struct D1SubscriptionRepo {
    pub db: Db,
}

const SUB_UPSERT: &str = "INSERT INTO subscriptions (id, user_id, status, started_at, expires_at, grace_until) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
     ON CONFLICT (user_id) DO UPDATE SET id = excluded.id, status = excluded.status, \
     started_at = excluded.started_at, expires_at = excluded.expires_at, grace_until = excluded.grace_until";
const SUB_SELECT: &str =
    "SELECT id, user_id, status, started_at, expires_at, grace_until FROM subscriptions";
const SUB_SET_STATUS: &str = "UPDATE subscriptions SET status = ?2, grace_until = ?3 WHERE id = ?1";

#[derive(serde::Deserialize)]
struct SubscriptionRow {
    id: String,
    user_id: String,
    status: String,
    started_at: String,
    expires_at: String,
    grace_until: Option<String>,
}

fn subscription_row(r: SubscriptionRow) -> Subscription {
    Subscription {
        id: parse_uid(&r.id),
        user_id: parse_uid(&r.user_id),
        status: SubscriptionStatus::parse(&r.status).unwrap_or(SubscriptionStatus::Active),
        started_at: parse_ts(&r.started_at),
        expires_at: parse_ts(&r.expires_at),
        grace_until: r.grace_until.map(|g| parse_ts(&g)),
    }
}

#[async_trait]
impl SubscriptionRepo for D1SubscriptionRepo {
    async fn find_by_user(&self, user_id: Uuid) -> Result<Option<Subscription>, String> {
        query_one(
            &self.db,
            &format!("{SUB_SELECT} WHERE user_id = ?1"),
            &[js_uid(user_id)],
        )
        .await
        .map(|r: Option<SubscriptionRow>| r.map(subscription_row))
    }

    async fn upsert(&self, sub: &Subscription) -> Result<(), String> {
        exec(
            &self.db,
            SUB_UPSERT,
            &[
                js_uid(sub.id),
                js_uid(sub.user_id),
                js_text(sub.status.as_str()),
                js_ts(sub.started_at),
                js_ts(sub.expires_at),
                match sub.grace_until {
                    Some(g) => js_ts(g),
                    None => JsValue::NULL,
                },
            ],
        )
        .await
    }

    async fn list_to_mark_grace(&self, now: DateTime<Utc>) -> Result<Vec<Subscription>, String> {
        query_all(
            &self.db,
            &format!("{SUB_SELECT} WHERE status = 'active' AND expires_at <= ?1"),
            &[js_ts(now)],
        )
        .await
        .map(|rows: Vec<SubscriptionRow>| rows.into_iter().map(subscription_row).collect())
    }

    async fn list_to_mark_lapsed(&self, now: DateTime<Utc>) -> Result<Vec<Subscription>, String> {
        query_all(
            &self.db,
            &format!("{SUB_SELECT} WHERE status = 'grace' AND grace_until IS NOT NULL AND grace_until <= ?1"),
            &[js_ts(now)],
        )
        .await
        .map(|rows: Vec<SubscriptionRow>| rows.into_iter().map(subscription_row).collect())
    }

    async fn list_to_delete(&self, now: DateTime<Utc>) -> Result<Vec<Subscription>, String> {
        query_all(
            &self.db,
            &format!("{SUB_SELECT} WHERE status IN ('lapsed', 'deleted') AND grace_until IS NOT NULL AND grace_until <= ?1"),
            &[js_ts(now)],
        )
        .await
        .map(|rows: Vec<SubscriptionRow>| rows.into_iter().map(subscription_row).collect())
    }

    async fn set_status(
        &self,
        id: Uuid,
        status: SubscriptionStatus,
        grace_until: Option<DateTime<Utc>>,
    ) -> Result<(), String> {
        exec(
            &self.db,
            SUB_SET_STATUS,
            &[
                js_uid(id),
                js_text(status.as_str()),
                match grace_until {
                    Some(g) => js_ts(g),
                    None => JsValue::NULL,
                },
            ],
        )
        .await
    }
}

// --- payment orders ---

pub struct D1PaymentOrderRepo {
    pub db: Db,
}

const ORDER_INSERT: &str = "INSERT INTO payment_orders \
     (id, user_id, amount, mint, recipient, status, signature, created_at, paid_at) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";
const ORDER_SELECT: &str = "SELECT id, user_id, amount, mint, recipient, status, signature, created_at, paid_at FROM payment_orders";
const ORDER_MARK_PAID: &str =
    "UPDATE payment_orders SET status = 'paid', signature = ?2, paid_at = ?3 \
     WHERE id = ?1 AND status = 'pending'";

#[derive(serde::Deserialize)]
struct OrderRow {
    id: String,
    user_id: String,
    #[serde(deserialize_with = "num_str")]
    amount: String,
    mint: String,
    recipient: String,
    status: String,
    signature: Option<String>,
    created_at: String,
    paid_at: Option<String>,
}

fn order_row(r: OrderRow) -> PaymentOrder {
    PaymentOrder {
        id: parse_uid(&r.id),
        user_id: parse_uid(&r.user_id),
        amount: parse_dec(&r.amount),
        mint: r.mint,
        recipient: r.recipient,
        status: PaymentOrderStatus::parse(&r.status).unwrap_or(PaymentOrderStatus::Pending),
        signature: r.signature,
        created_at: parse_ts(&r.created_at),
        paid_at: r.paid_at.map(|p| parse_ts(&p)),
    }
}

#[async_trait]
impl PaymentOrderRepo for D1PaymentOrderRepo {
    async fn insert(&self, order: &PaymentOrder) -> Result<(), String> {
        exec(
            &self.db,
            ORDER_INSERT,
            &[
                js_uid(order.id),
                js_uid(order.user_id),
                js_dec(order.amount),
                js_text(&order.mint),
                js_text(&order.recipient),
                js_text(order.status.as_str()),
                js_opt_text(order.signature.as_deref()),
                js_ts(order.created_at),
                match order.paid_at {
                    Some(p) => js_ts(p),
                    None => JsValue::NULL,
                },
            ],
        )
        .await
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<PaymentOrder>, String> {
        query_one(
            &self.db,
            &format!("{ORDER_SELECT} WHERE id = ?1"),
            &[js_uid(id)],
        )
        .await
        .map(|r: Option<OrderRow>| r.map(order_row))
    }

    async fn find_by_signature(&self, signature: &str) -> Result<Option<PaymentOrder>, String> {
        query_one(
            &self.db,
            &format!("{ORDER_SELECT} WHERE signature = ?1"),
            &[js_text(signature)],
        )
        .await
        .map(|r: Option<OrderRow>| r.map(order_row))
    }

    async fn mark_paid_if_pending(
        &self,
        id: Uuid,
        signature: &str,
        paid_at: DateTime<Utc>,
    ) -> Result<bool, String> {
        let changed = exec_count(
            &self.db,
            ORDER_MARK_PAID,
            &[js_uid(id), js_text(signature), js_ts(paid_at)],
        )
        .await?;
        Ok(changed == 1)
    }
}

// --- storage usage ---

pub struct D1StorageRepo {
    pub db: Db,
}

const STORAGE_FILE_SELECT: &str = "SELECT bytes FROM storage_files WHERE shop_id = ?1 AND key = ?2";
const STORAGE_FILE_INSERT: &str =
    "INSERT INTO storage_files (shop_id, key, bytes) VALUES (?1, ?2, ?3)";
const STORAGE_TOTAL_SELECT: &str = "SELECT bytes FROM storage_usage WHERE shop_id = ?1";
const STORAGE_TOTAL_UPSERT: &str = "INSERT INTO storage_usage (shop_id, bytes) VALUES (?1, ?2) \
     ON CONFLICT (shop_id) DO UPDATE SET bytes = excluded.bytes";

#[async_trait]
impl StorageUsageRepo for D1StorageRepo {
    async fn add_checked(
        &self,
        shop_id: Uuid,
        key: Uuid,
        bytes: i64,
        quota: i64,
    ) -> Result<i64, StorageError> {
        let existing: Option<BytesRow> = query_one(
            &self.db,
            STORAGE_FILE_SELECT,
            &[js_uid(shop_id), js_uid(key)],
        )
        .await?;
        let total = self.total(shop_id).await?;
        if existing.is_some() {
            return Ok(total);
        }
        if total.saturating_add(bytes) > quota {
            return Err(StorageError::QuotaExceeded);
        }
        exec(
            &self.db,
            STORAGE_FILE_INSERT,
            &[js_uid(shop_id), js_uid(key), js_i64(bytes)],
        )
        .await?;
        exec(
            &self.db,
            STORAGE_TOTAL_UPSERT,
            &[js_uid(shop_id), js_i64(total + bytes)],
        )
        .await?;
        Ok(total + bytes)
    }

    async fn total(&self, shop_id: Uuid) -> Result<i64, StorageError> {
        let row: Option<BytesRow> =
            query_one(&self.db, STORAGE_TOTAL_SELECT, &[js_uid(shop_id)]).await?;
        Ok(row.map(|r| r.bytes).unwrap_or(0))
    }

    async fn list(&self, shop_id: Uuid) -> Result<Vec<(Uuid, i64)>, StorageError> {
        #[derive(serde::Deserialize)]
        struct Row {
            key: String,
            bytes: i64,
        }
        let rows: Vec<Row> = query_all(
            &self.db,
            "SELECT key, bytes FROM storage_files WHERE shop_id = ?1 ORDER BY key",
            &[js_uid(shop_id)],
        )
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| Uuid::parse_str(&r.key).ok().map(|k| (k, r.bytes)))
            .collect())
    }

    async fn remove(&self, shop_id: Uuid, key: Uuid) -> Result<i64, StorageError> {
        exec(
            &self.db,
            "DELETE FROM storage_files WHERE shop_id = ?1 AND key = ?2",
            &[js_uid(shop_id), js_uid(key)],
        )
        .await?;
        let row: Option<BytesRow> =
            query_one(&self.db, STORAGE_TOTAL_SELECT, &[js_uid(shop_id)]).await?;
        let total: i64 = row.map(|r| r.bytes).unwrap_or(0);
        exec(
            &self.db,
            STORAGE_TOTAL_UPSERT,
            &[js_uid(shop_id), js_i64(total)],
        )
        .await?;
        Ok(total)
    }
}

#[derive(serde::Deserialize)]
struct BytesRow {
    bytes: i64,
}
