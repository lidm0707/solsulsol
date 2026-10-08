//! D1 repositories implementing the pos-port storage traits.
//!
//! D1 (SQLite) has no interactive transactions, so `InvoiceRepo::insert`
//! writes the invoice header + lines as one atomic `batch()`. Decimals and
//! timestamps are stored as TEXT and parsed back on read.

use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pos_app::domain::entities::{
    Invite, InviteStatus, Invoice, InvoiceLine, InvoiceStatus, PaymentDestination, Product, Shop,
    ShopMember, User, VatMode,
};
use pos_app::domain::value_objects::{
    Chain, ChainAddress, Email, Money, PasswordHash, Role, SplToken,
};
use pos_app::port::database::{
    DestinationRepo, InviteRepo, InvoiceRepo, ProductRepo, ShopMemberRepo, ShopRepo, UserRepo,
};
use rust_decimal::Decimal;
use uuid::Uuid;
use worker::console_error;
use worker::wasm_bindgen::JsValue;
use worker::D1Database;

pub const NIL: Uuid = Uuid::nil();
const FALLBACK_BTC_ADDRESS: &str = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";

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
    email: String,
    password_hash: String,
    role: String,
    created_at: String,
}

fn user_row(r: UserRow) -> User {
    User {
        id: parse_uid(&r.id),
        email: Email::parse(&r.email).unwrap_or_else(|_| Email::unchecked(r.email)),
        password: PasswordHash::from_hash(r.password_hash),
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
}

fn shop_row(r: ShopRow) -> Shop {
    Shop {
        id: parse_uid(&r.id),
        owner_id: parse_uid(&r.owner_id),
        name: r.name,
        vat_mode: VatMode::parse(&r.vat_mode).unwrap_or_default(),
        vat_rate: parse_dec(&r.vat_rate),
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
struct ProductRow {
    id: String,
    shop_id: String,
    name: String,
    #[serde(deserialize_with = "num_str")]
    price: String,
    #[serde(deserialize_with = "num_str")]
    cost: String,
    units: i64,
    barcode: Option<String>,
}

fn product_row(r: ProductRow) -> Product {
    let price = parse_dec(&r.price);
    Product {
        id: parse_uid(&r.id),
        shop_id: parse_uid(&r.shop_id),
        name: r.name,
        price: Money::new(price).unwrap_or_else(|_| Money::one()),
        cost: parse_dec(&r.cost),
        units: r.units,
        barcode: r.barcode,
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
    let chain = Chain::parse(&r.chain).unwrap_or(Chain::Binance);
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
struct InvoiceHeadRow {
    id: String,
    shop_id: String,
    status: String,
    destination_id: String,
    vat_mode: String,
    #[serde(deserialize_with = "num_str")]
    vat_rate: String,
}

fn head_row(r: InvoiceHeadRow) -> (Uuid, Uuid, InvoiceStatus, Uuid, VatMode, Decimal) {
    (
        parse_uid(&r.id),
        parse_uid(&r.shop_id),
        InvoiceStatus::parse(&r.status).unwrap_or(InvoiceStatus::Pending),
        parse_uid(&r.destination_id),
        VatMode::parse(&r.vat_mode).unwrap_or_default(),
        parse_dec(&r.vat_rate),
    )
}

#[derive(serde::Deserialize)]
struct LineRow {
    product_id: String,
    name: String,
    #[serde(deserialize_with = "num_str")]
    price: String,
    #[serde(deserialize_with = "num_str")]
    cost: String,
    qty: i64,
}

fn line_row(r: LineRow) -> InvoiceLine {
    let price = parse_dec(&r.price);
    InvoiceLine {
        product_id: parse_uid(&r.product_id),
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

const USER_INSERT: &str =
    "INSERT INTO users (id, email, password_hash, role, created_at) VALUES (?1, ?2, ?3, ?4, ?5)";
const USER_SELECT: &str = "SELECT id, email, password_hash, role, created_at FROM users";

#[async_trait]
impl UserRepo for D1UserRepo {
    async fn insert(&self, user: &User) -> Result<(), String> {
        exec(
            &self.db,
            USER_INSERT,
            &[
                js_uid(user.id),
                js_text(user.email.as_str()),
                js_text(user.password.as_str()),
                js_text(user.role.as_str()),
                js_ts(user.created_at),
            ],
        )
        .await
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, String> {
        query_one(
            &self.db,
            &format!("{USER_SELECT} WHERE email = ?1"),
            &[js_text(email)],
        )
        .await
        .map(|r: Option<UserRow>| r.map(user_row))
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
}

// --- shops ---

pub struct D1ShopRepo {
    pub db: Db,
}

const SHOP_INSERT: &str =
    "INSERT INTO shops (id, owner_id, name, vat_mode, vat_rate) VALUES (?1, ?2, ?3, ?4, ?5)";
const SHOP_SELECT: &str = "SELECT id, owner_id, name, vat_mode, vat_rate FROM shops";

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
            ],
        )
        .await
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Shop>, String> {
        query_one(
            &self.db,
            &format!("{SHOP_SELECT} WHERE id = ?1"),
            &[js_uid(id)],
        )
        .await
        .map(|r: Option<ShopRow>| r.map(shop_row))
    }

    async fn find_for_user(&self, user_id: Uuid) -> Result<Vec<Shop>, String> {
        query_all(
            &self.db,
            "SELECT s.id, s.owner_id, s.name, s.vat_mode, s.vat_rate FROM shops s \
             JOIN shop_members m ON m.shop_id = s.id WHERE m.user_id = ?1",
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
    ) -> Result<(), String> {
        exec(
            &self.db,
            "UPDATE shops SET vat_mode = ?2, vat_rate = ?3 WHERE id = ?1",
            &[js_uid(id), js_text(vat_mode.as_str()), js_dec(vat_rate)],
        )
        .await
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

// --- products ---

pub struct D1ProductRepo {
    pub db: Db,
}

const PRODUCT_INSERT: &str =
    "INSERT INTO products (id, shop_id, name, price, cost, units, barcode) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
const PRODUCT_SELECT: &str = "SELECT id, shop_id, name, price, cost, units, barcode FROM products";

#[async_trait]
impl ProductRepo for D1ProductRepo {
    async fn insert(&self, product: &Product) -> Result<(), String> {
        exec(
            &self.db,
            PRODUCT_INSERT,
            &[
                js_uid(product.id),
                js_uid(product.shop_id),
                js_text(&product.name),
                js_dec(product.price.amount()),
                js_dec(product.cost),
                js_i64(product.units),
                match &product.barcode {
                    Some(b) => js_text(b),
                    None => JsValue::NULL,
                },
            ],
        )
        .await
    }

    async fn find_by_shop(&self, shop_id: Uuid) -> Result<Vec<Product>, String> {
        query_all(
            &self.db,
            &format!("{PRODUCT_SELECT} WHERE shop_id = ?1"),
            &[js_uid(shop_id)],
        )
        .await
        .map(|rows: Vec<ProductRow>| rows.into_iter().map(product_row).collect())
    }

    async fn take_units(&self, product_id: Uuid) -> Result<(), String> {
        let stmt = self
            .db
            .prepare("UPDATE products SET units = units - 1 WHERE id = ?1 AND units > 0")
            .bind(&[js_uid(product_id)])
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
            return Err(String::from("product out of stock"));
        }
        Ok(())
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

// --- invoices ---

pub struct D1InvoiceRepo {
    pub db: Db,
}

const INVOICE_INSERT: &str =
    "INSERT INTO invoices (id, shop_id, status, destination_id, vat_mode, vat_rate) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";
const LINE_INSERT: &str =
    "INSERT INTO invoice_lines (invoice_id, product_id, name, price, cost, qty) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";
const INVOICE_SELECT: &str =
    "SELECT id, shop_id, status, destination_id, vat_mode, vat_rate FROM invoices";

impl D1InvoiceRepo {
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
            return ChainAddress::parse(Chain::Btc, FALLBACK_BTC_ADDRESS)
                .map(|addr| PaymentDestination {
                    id: NIL,
                    shop_id: NIL,
                    address: addr,
                    token: SplToken::Sol,
                })
                .map_err(|_| String::from("const fallback address must parse"));
        };
        let chain = Chain::parse(&r.chain).unwrap_or(Chain::Binance);
        Ok(PaymentDestination {
            id: parse_uid(&r.id),
            shop_id: NIL,
            address: ChainAddress::parse(chain, &r.address)
                .unwrap_or_else(|_| ChainAddress::unchecked(chain, r.address)),
            token: SplToken::parse(&r.token).unwrap_or_default(),
        })
    }

    async fn fetch_lines(&self, invoice_id: Uuid) -> Result<Vec<InvoiceLine>, String> {
        let rows: Vec<LineRow> = query_all(
            &self.db,
            "SELECT product_id, name, price, cost, qty FROM invoice_lines WHERE invoice_id = ?1",
            &[js_uid(invoice_id)],
        )
        .await?;
        Ok(rows.into_iter().map(line_row).collect())
    }
}

#[async_trait]
impl InvoiceRepo for D1InvoiceRepo {
    async fn insert(&self, invoice: &Invoice) -> Result<(), String> {
        let head = self
            .db
            .prepare(INVOICE_INSERT)
            .bind(&[
                js_uid(invoice.id),
                js_uid(invoice.shop_id),
                js_text(invoice.status.as_str()),
                js_uid(invoice.destination.id),
                js_text(invoice.vat_mode.as_str()),
                js_dec(invoice.vat_rate),
            ])
            .map_err(err)?;
        let mut stmts = Vec::with_capacity(invoice.lines.len() + 1);
        stmts.push(head);
        for l in &invoice.lines {
            let stmt = self
                .db
                .prepare(LINE_INSERT)
                .bind(&[
                    js_uid(invoice.id),
                    js_uid(l.product_id),
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

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Invoice>, String> {
        let Some(head) = query_one(
            &self.db,
            &format!("{INVOICE_SELECT} WHERE id = ?1"),
            &[js_uid(id)],
        )
        .await
        .map(|r: Option<InvoiceHeadRow>| r.map(head_row))?
        else {
            return Ok(None);
        };
        let (id, shop_id, status, dest_id, vat_mode, vat_rate) = head;
        let destination = self.fetch_destination(dest_id).await?;
        let lines = self.fetch_lines(id).await?;
        Ok(Some(Invoice {
            id,
            shop_id,
            status,
            lines,
            destination,
            vat_mode,
            vat_rate,
        }))
    }

    async fn list_by_shop(&self, shop_id: Uuid) -> Result<Vec<Invoice>, String> {
        let heads: Vec<InvoiceHeadRow> = query_all(
            &self.db,
            &format!("{INVOICE_SELECT} WHERE shop_id = ?1"),
            &[js_uid(shop_id)],
        )
        .await?;
        let mut out = Vec::with_capacity(heads.len());
        for head in heads {
            let (id, shop_id, status, dest_id, vat_mode, vat_rate) = head_row(head);
            let destination = self.fetch_destination(dest_id).await?;
            let lines = self.fetch_lines(id).await?;
            out.push(Invoice {
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
}
