//! Sqlx Postgres adapters implementing the storage ports.
//! Single-table queries only; joins are combined in the app layer.

use async_trait::async_trait;
use sqlx::PgPool;

use crate::domain::entities::{
    Invite, InviteStatus, Invoice, InvoiceLine, InvoiceStatus, PaymentDestination, Product, Shop,
    ShopMember, User, VatMode,
};
use crate::domain::value_objects::{Chain, ChainAddress, Email, Money, PasswordHash, Role, SplToken};
use crate::port::{
    DestinationRepo, InviteRepo, InvoiceRepo, ProductRepo, ShopMemberRepo, ShopRepo, Tx, TxManager,
    UserRepo,
};
use rust_decimal::Decimal;

pub const DEFAULT_MAX_CONNECTIONS: u32 = 8;
const NIL: uuid::Uuid = uuid::Uuid::nil();

pub async fn connect(url: &str) -> Result<PgPool, sqlx::Error> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(DEFAULT_MAX_CONNECTIONS)
        .connect(url)
        .await
}

pub struct PgUserRepo {
    pub pool: PgPool,
}

#[async_trait]
impl UserRepo for PgUserRepo {
    async fn insert(&self, user: &User) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO users (id, email, password_hash, role, created_at) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(user.id)
        .bind(user.email.as_str())
        .bind(user.password.as_str())
        .bind(user.role.as_str())
        .bind(user.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, String> {
        let row: Option<(
            uuid::Uuid,
            String,
            String,
            String,
            chrono::DateTime<chrono::Utc>,
        )> = sqlx::query_as(
            "SELECT id, email, password_hash, role, created_at FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(row.map(user_row))
    }

    async fn find_by_id(&self, id: uuid::Uuid) -> Result<Option<User>, String> {
        let row: Option<(
            uuid::Uuid,
            String,
            String,
            String,
            chrono::DateTime<chrono::Utc>,
        )> = sqlx::query_as(
            "SELECT id, email, password_hash, role, created_at FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(row.map(user_row))
    }
}

type UserTuple = (
    uuid::Uuid,
    String,
    String,
    String,
    chrono::DateTime<chrono::Utc>,
);

fn user_row(row: UserTuple) -> User {
    let (id, email, password_hash, role, created_at) = row;
    User {
        id,
        email: Email::parse(&email).unwrap_or_else(|_| Email::unchecked(email)),
        password: PasswordHash::from_hash(password_hash),
        role: Role::parse(&role).unwrap_or(Role::Employee),
        created_at,
    }
}

pub struct PgShopRepo {
    pub pool: PgPool,
}

type ShopTuple = (uuid::Uuid, uuid::Uuid, String, String, Decimal);

fn shop_row(row: ShopTuple) -> Shop {
    let (id, owner_id, name, vat_mode, vat_rate) = row;
    Shop {
        id,
        owner_id,
        name,
        vat_mode: VatMode::parse(&vat_mode).unwrap_or_default(),
        vat_rate,
    }
}

#[async_trait]
impl ShopRepo for PgShopRepo {
    async fn insert(&self, shop: &Shop) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO shops (id, owner_id, name, vat_mode, vat_rate) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(shop.id)
        .bind(shop.owner_id)
        .bind(&shop.name)
        .bind(shop.vat_mode.as_str())
        .bind(shop.vat_rate)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find_by_id(&self, id: uuid::Uuid) -> Result<Option<Shop>, String> {
        let row: Option<ShopTuple> = sqlx::query_as(
            "SELECT id, owner_id, name, vat_mode, vat_rate FROM shops WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(row.map(shop_row))
    }

    async fn find_for_user(&self, user_id: uuid::Uuid) -> Result<Vec<Shop>, String> {
        let rows: Vec<ShopTuple> = sqlx::query_as(
            "SELECT s.id, s.owner_id, s.name, s.vat_mode, s.vat_rate FROM shops s \
             JOIN shop_members m ON m.shop_id = s.id WHERE m.user_id = $1",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(shop_row).collect())
    }

    async fn update_settings(
        &self,
        id: uuid::Uuid,
        vat_mode: VatMode,
        vat_rate: Decimal,
    ) -> Result<(), String> {
        sqlx::query("UPDATE shops SET vat_mode = $2, vat_rate = $3 WHERE id = $1")
            .bind(id)
            .bind(vat_mode.as_str())
            .bind(vat_rate)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub struct PgMemberRepo {
    pub pool: PgPool,
}

type MemberTuple = (uuid::Uuid, uuid::Uuid, String);

fn member_row(row: MemberTuple) -> ShopMember {
    let (shop_id, user_id, role) = row;
    ShopMember {
        shop_id,
        user_id,
        role: Role::parse(&role).unwrap_or(Role::Employee),
    }
}

#[async_trait]
impl ShopMemberRepo for PgMemberRepo {
    async fn insert(&self, member: &ShopMember) -> Result<(), String> {
        sqlx::query("INSERT INTO shop_members (shop_id, user_id, role) VALUES ($1, $2, $3)")
            .bind(member.shop_id)
            .bind(member.user_id)
            .bind(member.role.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find_role(
        &self,
        shop_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> Result<Option<Role>, String> {
        let role: Option<(String,)> =
            sqlx::query_as("SELECT role FROM shop_members WHERE shop_id = $1 AND user_id = $2")
                .bind(shop_id)
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        Ok(role.and_then(|r| Role::parse(&r.0)))
    }

    async fn list_by_shop(&self, shop_id: uuid::Uuid) -> Result<Vec<ShopMember>, String> {
        let rows: Vec<MemberTuple> =
            sqlx::query_as("SELECT shop_id, user_id, role FROM shop_members WHERE shop_id = $1")
                .bind(shop_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(member_row).collect())
    }

    async fn update_role(
        &self,
        shop_id: uuid::Uuid,
        user_id: uuid::Uuid,
        role: Role,
    ) -> Result<(), String> {
        sqlx::query("UPDATE shop_members SET role = $3 WHERE shop_id = $1 AND user_id = $2")
            .bind(shop_id)
            .bind(user_id)
            .bind(role.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub struct PgProductRepo {
    pub pool: PgPool,
}

type ProductTuple = (
    uuid::Uuid,
    uuid::Uuid,
    String,
    rust_decimal::Decimal,
    rust_decimal::Decimal,
    i64,
    Option<String>,
);

fn product_row(row: ProductTuple) -> Product {
    let (id, shop_id, name, price, cost, units, barcode) = row;
    Product {
        id,
        shop_id,
        name,
        price: Money::new(price).unwrap_or_else(|_| Money::one()),
        cost,
        units,
        barcode,
    }
}

#[async_trait]
impl ProductRepo for PgProductRepo {
    async fn insert(&self, product: &Product) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO products (id, shop_id, name, price, cost, units, barcode) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(product.id)
        .bind(product.shop_id)
        .bind(&product.name)
        .bind(product.price.amount())
        .bind(product.cost)
        .bind(product.units)
        .bind(&product.barcode)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find_by_shop(&self, shop_id: uuid::Uuid) -> Result<Vec<Product>, String> {
        let rows: Vec<ProductTuple> = sqlx::query_as(
            "SELECT id, shop_id, name, price, cost, units, barcode FROM products WHERE shop_id = $1",
        )
        .bind(shop_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(product_row).collect())
    }

    async fn take_units(&self, product_id: uuid::Uuid) -> Result<(), String> {
        let res = sqlx::query("UPDATE products SET units = units - 1 WHERE id = $1 AND units > 0")
            .bind(product_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        if res.rows_affected() == 0 {
            return Err(String::from("product out of stock"));
        }
        Ok(())
    }
}

pub struct PgDestinationRepo {
    pub pool: PgPool,
}

type DestinationTuple = (uuid::Uuid, uuid::Uuid, String, String, String);

fn destination_row(row: DestinationTuple) -> PaymentDestination {
    let (id, shop_id, chain, address, token) = row;
    let chain = Chain::parse(&chain).unwrap_or(Chain::Binance);
    PaymentDestination {
        id,
        shop_id,
        address: ChainAddress::parse(chain, &address)
            .unwrap_or_else(|_| ChainAddress::unchecked(chain, address)),
        token: SplToken::parse(&token).unwrap_or_default(),
    }
}

#[async_trait]
impl DestinationRepo for PgDestinationRepo {
    async fn insert(&self, d: &PaymentDestination) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO destinations (id, shop_id, chain, address, token) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(d.id)
        .bind(d.shop_id)
        .bind(d.address.chain().as_str())
        .bind(d.address.as_str())
        .bind(d.token.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find_by_id(&self, id: uuid::Uuid) -> Result<Option<PaymentDestination>, String> {
        let row: Option<DestinationTuple> =
            sqlx::query_as("SELECT id, shop_id, chain, address, token FROM destinations WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        Ok(row.map(destination_row))
    }

    async fn list_by_shop(&self, shop_id: uuid::Uuid) -> Result<Vec<PaymentDestination>, String> {
        let rows: Vec<DestinationTuple> = sqlx::query_as(
            "SELECT id, shop_id, chain, address, token FROM destinations WHERE shop_id = $1",
        )
        .bind(shop_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(destination_row).collect())
    }

    async fn delete(&self, id: uuid::Uuid) -> Result<(), String> {
        sqlx::query("DELETE FROM destinations WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub struct PgInviteRepo {
    pub pool: PgPool,
}

type InviteTuple = (
    uuid::Uuid,
    uuid::Uuid,
    String,
    String,
    String,
    chrono::DateTime<chrono::Utc>,
);

fn invite_row(row: InviteTuple) -> Invite {
    let (id, shop_id, email, role, status, expires_at) = row;
    Invite {
        id,
        shop_id,
        email: Email::parse(&email).unwrap_or_else(|_| Email::unchecked(email)),
        role: Role::parse(&role).unwrap_or(Role::Employee),
        status: InviteStatus::parse(&status).unwrap_or(InviteStatus::Revoked),
        expires_at,
    }
}

#[async_trait]
impl InviteRepo for PgInviteRepo {
    async fn insert(&self, invite: &Invite) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO invites (id, shop_id, email, role, status, expires_at) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(invite.id)
        .bind(invite.shop_id)
        .bind(invite.email.as_str())
        .bind(invite.role.as_str())
        .bind(invite.status.as_str())
        .bind(invite.expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find_by_id(&self, id: uuid::Uuid) -> Result<Option<Invite>, String> {
        let row: Option<InviteTuple> = sqlx::query_as(
            "SELECT id, shop_id, email, role, status, expires_at FROM invites WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(row.map(invite_row))
    }

    async fn list_by_shop(&self, shop_id: uuid::Uuid) -> Result<Vec<Invite>, String> {
        let rows: Vec<InviteTuple> = sqlx::query_as(
            "SELECT id, shop_id, email, role, status, expires_at FROM invites WHERE shop_id = $1",
        )
        .bind(shop_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(invite_row).collect())
    }

    async fn set_status(&self, id: uuid::Uuid, status: InviteStatus) -> Result<(), String> {
        sqlx::query("UPDATE invites SET status = $2 WHERE id = $1")
            .bind(id)
            .bind(status.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub struct PgInvoiceRepo {
    pub pool: PgPool,
}

type InvoiceTuple = (uuid::Uuid, uuid::Uuid, String, uuid::Uuid, String, Decimal);

fn invoice_row(
    row: InvoiceTuple,
) -> (
    uuid::Uuid,
    uuid::Uuid,
    InvoiceStatus,
    uuid::Uuid,
    VatMode,
    Decimal,
) {
    let (id, shop_id, status, destination_id, vat_mode, vat_rate) = row;
    let status = InvoiceStatus::parse(&status).unwrap_or(InvoiceStatus::Pending);
    let vat_mode = VatMode::parse(&vat_mode).unwrap_or_default();
    (id, shop_id, status, destination_id, vat_mode, vat_rate)
}

#[async_trait]
impl InvoiceRepo for PgInvoiceRepo {
    async fn insert(&self, invoice: &Invoice) -> Result<(), String> {
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT INTO invoices (id, shop_id, status, destination_id, vat_mode, vat_rate) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(invoice.id)
        .bind(invoice.shop_id)
        .bind(invoice.status.as_str())
        .bind(invoice.destination.id)
        .bind(invoice.vat_mode.as_str())
        .bind(invoice.vat_rate)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        for line in &invoice.lines {
            sqlx::query(
                "INSERT INTO invoice_lines (invoice_id, product_id, name, price, cost, qty) VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(invoice.id)
            .bind(line.product_id)
            .bind(&line.name)
            .bind(line.price.amount())
            .bind(line.cost)
            .bind(i64::from(line.qty))
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        }
        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find_by_id(&self, id: uuid::Uuid) -> Result<Option<Invoice>, String> {
        let row: Option<InvoiceTuple> = sqlx::query_as(
            "SELECT id, shop_id, status, destination_id, vat_mode, vat_rate FROM invoices WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        let Some((id, shop_id, status, dest_id, vat_mode, vat_rate)) = row.map(invoice_row) else {
            return Ok(None);
        };
        let destination = fetch_destination(&self.pool, dest_id).await?;
        let lines = fetch_lines(&self.pool, id).await?;
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

    async fn list_by_shop(&self, shop_id: uuid::Uuid) -> Result<Vec<Invoice>, String> {
        let rows: Vec<InvoiceTuple> = sqlx::query_as(
            "SELECT id, shop_id, status, destination_id, vat_mode, vat_rate FROM invoices WHERE shop_id = $1",
        )
        .bind(shop_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity(rows.len());
        for (id, _shop_id, status, dest_id, vat_mode, vat_rate) in rows.into_iter().map(invoice_row)
        {
            let destination = fetch_destination(&self.pool, dest_id).await?;
            let lines = fetch_lines(&self.pool, id).await?;
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

async fn fetch_destination(
    pool: &sqlx::PgPool,
    id: uuid::Uuid,
) -> Result<PaymentDestination, String> {
    let row: Option<(uuid::Uuid, String, String, String)> =
        sqlx::query_as("SELECT id, chain, address, token FROM destinations WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    let Some((dest_id, chain, address, token)) = row else {
        return ChainAddress::parse(Chain::Btc, "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa")
            .map(|addr| PaymentDestination {
                id: NIL,
                shop_id: NIL,
                address: addr,
                token: SplToken::Sol,
            })
            .map_err(|_| String::from("const fallback address must parse"));
    };
    let chain = Chain::parse(&chain).unwrap_or(Chain::Binance);
    Ok(PaymentDestination {
        id: dest_id,
        shop_id: NIL,
        address: ChainAddress::parse(chain, &address)
            .unwrap_or_else(|_| ChainAddress::unchecked(chain, address)),
        token: SplToken::parse(&token).unwrap_or_default(),
    })
}

async fn fetch_lines(
    pool: &sqlx::PgPool,
    invoice_id: uuid::Uuid,
) -> Result<Vec<InvoiceLine>, String> {
    let rows: Vec<(
        uuid::Uuid,
        String,
        rust_decimal::Decimal,
        rust_decimal::Decimal,
        i64,
    )> = sqlx::query_as(
        "SELECT product_id, name, price, cost, qty FROM invoice_lines WHERE invoice_id = $1",
    )
    .bind(invoice_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|(product_id, name, price, cost, qty)| InvoiceLine {
            product_id,
            name,
            price: Money::new(price).unwrap_or_else(|_| Money::one()),
            cost,
            qty: u32::try_from(qty).unwrap_or(0),
        })
        .collect())
}

pub struct PgTxManager {
    pub pool: PgPool,
}

#[async_trait]
impl TxManager for PgTxManager {
    async fn begin(&self) -> Result<Box<dyn Tx>, String> {
        let tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        Ok(Box::new(PgTx { tx: Some(tx) }))
    }
}

struct PgTx {
    tx: Option<sqlx::Transaction<'static, sqlx::Postgres>>,
}

#[async_trait]
impl Tx for PgTx {
    async fn commit(mut self: Box<Self>) -> Result<(), String> {
        if let Some(tx) = self.tx.take() {
            tx.commit().await.map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    async fn rollback(mut self: Box<Self>) -> Result<(), String> {
        if let Some(tx) = self.tx.take() {
            tx.rollback().await.map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
