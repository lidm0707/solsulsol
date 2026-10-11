//! Sqlx Postgres adapters implementing the storage ports.
//! Single-table queries only; joins are combined in the app layer.

use async_trait::async_trait;
use sqlx::PgPool;

use crate::domain::entities::subscription::{
    AuthNonce, PaymentOrder, PaymentOrderStatus, Subscription, SubscriptionStatus,
};
use crate::domain::entities::{
    Billing, BillingLine, BillingStatus, Currency, Goods, Invite, InviteStatus, PaymentDestination,
    Shop, ShopMember, User, VatMode,
};
use crate::domain::value_objects::{Chain, ChainAddress, Email, Money, Role, SplToken, Wallet};
use crate::domain::value_objects::{GoodsIcon, GoodsType, PastelColor};
use crate::port::database::{
    BillingRepo, DestinationRepo, GoodsRepo, InviteRepo, NonceRepo, PaymentOrderRepo,
    ShopMemberRepo, ShopRepo, StorageError, StorageUsageRepo, SubscriptionRepo, Tx, TxManager,
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

const USER_COLUMNS: &str = "id, email, wallet, role, created_at";

#[async_trait]
impl UserRepo for PgUserRepo {
    async fn insert(&self, user: &User) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO users (id, email, wallet, role, created_at) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(user.id)
        .bind(user.email.as_ref().map(Email::as_str))
        .bind(user.wallet.as_ref().map(Wallet::as_str))
        .bind(user.role.as_str())
        .bind(user.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find_by_id(&self, id: uuid::Uuid) -> Result<Option<User>, String> {
        let row: Option<UserTuple> =
            sqlx::query_as(&format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1"))
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        Ok(row.map(user_row))
    }

    async fn find_by_wallet(&self, wallet: &str) -> Result<Option<User>, String> {
        let row: Option<UserTuple> = sqlx::query_as(&format!(
            "SELECT {USER_COLUMNS} FROM users WHERE wallet = $1"
        ))
        .bind(wallet)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(row.map(user_row))
    }
}

type UserTuple = (
    uuid::Uuid,
    Option<String>,
    Option<String>,
    String,
    chrono::DateTime<chrono::Utc>,
);

fn user_row(row: UserTuple) -> User {
    let (id, email, wallet, role, created_at) = row;
    User {
        id,
        email: email.map(|e| Email::parse(&e).unwrap_or_else(|_| Email::unchecked(e))),
        wallet: wallet.map(|w| Wallet::parse(&w).unwrap_or_else(|_| Wallet::unchecked(w))),
        role: Role::parse(&role).unwrap_or(Role::Employee),
        created_at,
    }
}

pub struct PgShopRepo {
    pub pool: PgPool,
}

type ShopTuple = (
    uuid::Uuid,
    uuid::Uuid,
    String,
    String,
    Decimal,
    Decimal,
    String,
    Option<chrono::DateTime<chrono::Utc>>,
);

fn shop_row(row: ShopTuple) -> Shop {
    let (id, owner_id, name, vat_mode, vat_rate, fx_thb_per_usd, currency, deleted_at) = row;
    Shop {
        id,
        owner_id,
        name,
        vat_mode: VatMode::parse(&vat_mode).unwrap_or_default(),
        vat_rate,
        fx_thb_per_usd,
        currency: Currency::parse(&currency).unwrap_or_default(),
        deleted_at,
    }
}

#[async_trait]
impl ShopRepo for PgShopRepo {
    async fn insert(&self, shop: &Shop) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO shops (id, owner_id, name, vat_mode, vat_rate, fx_thb_per_usd, currency, deleted_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(shop.id)
        .bind(shop.owner_id)
        .bind(&shop.name)
        .bind(shop.vat_mode.as_str())
        .bind(shop.vat_rate)
        .bind(shop.fx_thb_per_usd)
        .bind(shop.currency.as_str())
        .bind(shop.deleted_at)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find_by_id(&self, id: uuid::Uuid) -> Result<Option<Shop>, String> {
        let row: Option<ShopTuple> = sqlx::query_as(
            "SELECT id, owner_id, name, vat_mode, vat_rate, fx_thb_per_usd, currency, deleted_at FROM shops \
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(row.map(shop_row))
    }

    async fn find_for_user(&self, user_id: uuid::Uuid) -> Result<Vec<Shop>, String> {
        let rows: Vec<ShopTuple> = sqlx::query_as(
            "SELECT s.id, s.owner_id, s.name, s.vat_mode, s.vat_rate, s.fx_thb_per_usd, s.currency, s.deleted_at \
             FROM shops s JOIN shop_members m ON m.shop_id = s.id \
             WHERE m.user_id = $1 AND s.deleted_at IS NULL",
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
        fx_thb_per_usd: Decimal,
        currency: Currency,
    ) -> Result<(), String> {
        sqlx::query(
            "UPDATE shops SET vat_mode = $2, vat_rate = $3, fx_thb_per_usd = $4, currency = $5 WHERE id = $1",
        )
        .bind(id)
        .bind(vat_mode.as_str())
        .bind(vat_rate)
        .bind(fx_thb_per_usd)
        .bind(currency.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn soft_delete_for_owner(
        &self,
        owner_id: uuid::Uuid,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, String> {
        let res = sqlx::query(
            "UPDATE shops SET deleted_at = $2 WHERE owner_id = $1 AND deleted_at IS NULL",
        )
        .bind(owner_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(res.rows_affected())
    }

    async fn hard_delete_expired(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, String> {
        let res = sqlx::query("DELETE FROM shops WHERE deleted_at IS NOT NULL AND deleted_at < $1")
            .bind(before)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(res.rows_affected())
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

pub struct PgGoodsRepo {
    pub pool: PgPool,
}

type GoodsTuple = (
    uuid::Uuid,
    uuid::Uuid,
    String,
    rust_decimal::Decimal,
    rust_decimal::Decimal,
    i64,
    Option<String>,
    String,
    String,
    String,
);

fn goods_row(row: GoodsTuple) -> Goods {
    let (id, shop_id, name, price, cost, units, barcode, icon, color, goods_type) = row;
    Goods {
        id,
        shop_id,
        name,
        price: Money::new(price).unwrap_or_else(|_| Money::one()),
        cost,
        units,
        barcode,
        icon: GoodsIcon::parse(&icon).unwrap_or_default(),
        color: PastelColor::parse(&color).unwrap_or_default(),
        goods_type: GoodsType::parse(&goods_type).unwrap_or_default(),
    }
}

#[async_trait]
impl GoodsRepo for PgGoodsRepo {
    async fn insert(&self, goods: &Goods) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO goods (id, shop_id, name, price, cost, units, barcode, icon, color, goods_type) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(goods.id)
        .bind(goods.shop_id)
        .bind(&goods.name)
        .bind(goods.price.amount())
        .bind(goods.cost)
        .bind(goods.units)
        .bind(&goods.barcode)
        .bind(goods.icon.as_str())
        .bind(goods.color.as_str())
        .bind(goods.goods_type.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find_by_shop(&self, shop_id: uuid::Uuid) -> Result<Vec<Goods>, String> {
        let rows: Vec<GoodsTuple> = sqlx::query_as(
            "SELECT id, shop_id, name, price, cost, units, barcode, icon, color, goods_type FROM goods WHERE shop_id = $1",
        )
        .bind(shop_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(goods_row).collect())
    }

    async fn take_units(&self, goods_id: uuid::Uuid) -> Result<(), String> {
        let res = sqlx::query("UPDATE goods SET units = units - 1 WHERE id = $1 AND units > 0")
            .bind(goods_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        if res.rows_affected() == 0 {
            return Err(String::from("goods out of stock"));
        }
        Ok(())
    }

    async fn find_by_id(&self, goods_id: uuid::Uuid) -> Result<Option<Goods>, String> {
        let rows: Vec<GoodsTuple> = sqlx::query_as(
            "SELECT id, shop_id, name, price, cost, units, barcode, icon, color, goods_type FROM goods WHERE id = $1",
        )
        .bind(goods_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().next().map(goods_row))
    }

    async fn delete(&self, goods_id: uuid::Uuid) -> Result<(), String> {
        sqlx::query("DELETE FROM goods WHERE id = $1")
            .bind(goods_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
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
        let row: Option<DestinationTuple> = sqlx::query_as(
            "SELECT id, shop_id, chain, address, token FROM destinations WHERE id = $1",
        )
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

pub struct PgBillingRepo {
    pub pool: PgPool,
}

type BillingTuple = (uuid::Uuid, uuid::Uuid, String, uuid::Uuid, String, Decimal);

fn billing_row(
    row: BillingTuple,
) -> (
    uuid::Uuid,
    uuid::Uuid,
    BillingStatus,
    uuid::Uuid,
    VatMode,
    Decimal,
) {
    let (id, shop_id, status, destination_id, vat_mode, vat_rate) = row;
    let status = BillingStatus::parse(&status).unwrap_or(BillingStatus::Pending);
    let vat_mode = VatMode::parse(&vat_mode).unwrap_or_default();
    (id, shop_id, status, destination_id, vat_mode, vat_rate)
}

#[async_trait]
impl BillingRepo for PgBillingRepo {
    async fn hard_delete_expired(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, String> {
        let res = sqlx::query(
            "DELETE FROM billings WHERE shop_id IN (SELECT id FROM shops WHERE deleted_at IS NOT NULL AND deleted_at < $1)",
        )
        .bind(before)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(res.rows_affected())
    }

    async fn insert(&self, billing: &Billing) -> Result<(), String> {
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT INTO billings (id, shop_id, status, destination_id, vat_mode, vat_rate) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(billing.id)
        .bind(billing.shop_id)
        .bind(billing.status.as_str())
        .bind(billing.destination.id)
        .bind(billing.vat_mode.as_str())
        .bind(billing.vat_rate)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        for line in &billing.lines {
            sqlx::query(
                "INSERT INTO billing_lines (billing_id, goods_id, name, price, cost, qty) VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(billing.id)
            .bind(line.goods_id)
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

    async fn find_by_id(&self, id: uuid::Uuid) -> Result<Option<Billing>, String> {
        let row: Option<BillingTuple> = sqlx::query_as(
            "SELECT id, shop_id, status, destination_id, vat_mode, vat_rate FROM billings WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        let Some((id, shop_id, status, dest_id, vat_mode, vat_rate)) = row.map(billing_row) else {
            return Ok(None);
        };
        let destination = fetch_destination(&self.pool, dest_id).await?;
        let lines = fetch_lines(&self.pool, id).await?;
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

    async fn list_by_shop(&self, shop_id: uuid::Uuid) -> Result<Vec<Billing>, String> {
        let rows: Vec<BillingTuple> = sqlx::query_as(
            "SELECT id, shop_id, status, destination_id, vat_mode, vat_rate FROM billings WHERE shop_id = $1",
        )
        .bind(shop_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity(rows.len());
        for (id, _shop_id, status, dest_id, vat_mode, vat_rate) in rows.into_iter().map(billing_row)
        {
            let destination = fetch_destination(&self.pool, dest_id).await?;
            let lines = fetch_lines(&self.pool, id).await?;
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

    async fn sales_by_goods(&self, shop_id: uuid::Uuid) -> Result<Vec<(uuid::Uuid, u64)>, String> {
        let rows: Vec<(uuid::Uuid, i64)> = sqlx::query_as(
            "SELECT l.goods_id, SUM(l.qty) FROM billing_lines l \
             JOIN billings i ON i.id = l.billing_id \
             WHERE i.shop_id = $1 AND i.status = 'paid' \
             GROUP BY l.goods_id ORDER BY SUM(l.qty) DESC",
        )
        .bind(shop_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(|(id, qty)| (id, u64::try_from(qty).unwrap_or(0)))
            .collect())
    }

    async fn goods_has_lines(&self, goods_id: uuid::Uuid) -> Result<bool, String> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM billing_lines WHERE goods_id = $1")
            .bind(goods_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(row.0 > 0)
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
                token: SplToken::default(),
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
    billing_id: uuid::Uuid,
) -> Result<Vec<BillingLine>, String> {
    let rows: Vec<(
        uuid::Uuid,
        String,
        rust_decimal::Decimal,
        rust_decimal::Decimal,
        i64,
    )> = sqlx::query_as(
        "SELECT goods_id, name, price, cost, qty FROM billing_lines WHERE billing_id = $1",
    )
    .bind(billing_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|(goods_id, name, price, cost, qty)| BillingLine {
            goods_id,
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

pub struct PgNonceRepo {
    pub pool: PgPool,
}

#[async_trait]
impl NonceRepo for PgNonceRepo {
    async fn insert(&self, nonce: &AuthNonce) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO auth_nonces (id, wallet, nonce, expires_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(nonce.id)
        .bind(&nonce.wallet)
        .bind(&nonce.nonce)
        .bind(nonce.expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find(&self, wallet: &str, nonce: &str) -> Result<Option<AuthNonce>, String> {
        let row: Option<(uuid::Uuid, String, String, chrono::DateTime<chrono::Utc>)> =
            sqlx::query_as(
                "SELECT id, wallet, nonce, expires_at FROM auth_nonces \
                 WHERE wallet = $1 AND nonce = $2 AND used_at IS NULL",
            )
            .bind(wallet)
            .bind(nonce)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(row.map(|(id, wallet, nonce, expires_at)| AuthNonce {
            id,
            wallet,
            nonce,
            expires_at,
        }))
    }

    async fn consume(
        &self,
        wallet: &str,
        nonce: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<bool, String> {
        let res = sqlx::query(
            "UPDATE auth_nonces SET used_at = $3 \
             WHERE wallet = $1 AND nonce = $2 AND used_at IS NULL AND expires_at > $3",
        )
        .bind(wallet)
        .bind(nonce)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(res.rows_affected() == 1)
    }
}

pub struct PgSubscriptionRepo {
    pub pool: PgPool,
}

type SubTuple = (
    uuid::Uuid,
    uuid::Uuid,
    String,
    chrono::DateTime<chrono::Utc>,
    chrono::DateTime<chrono::Utc>,
    Option<chrono::DateTime<chrono::Utc>>,
);

fn sub_row(row: SubTuple) -> Subscription {
    let (id, user_id, status, started_at, expires_at, grace_until) = row;
    Subscription {
        id,
        user_id,
        status: SubscriptionStatus::parse(&status).unwrap_or(SubscriptionStatus::Grace),
        started_at,
        expires_at,
        grace_until,
    }
}

const SUB_COLUMNS: &str = "id, user_id, status, started_at, expires_at, grace_until";

#[async_trait]
impl SubscriptionRepo for PgSubscriptionRepo {
    async fn find_by_user(&self, user_id: uuid::Uuid) -> Result<Option<Subscription>, String> {
        let row: Option<SubTuple> = sqlx::query_as(&format!(
            "SELECT {SUB_COLUMNS} FROM subscriptions WHERE user_id = $1"
        ))
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(row.map(sub_row))
    }

    async fn upsert(&self, sub: &Subscription) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO subscriptions (id, user_id, status, started_at, expires_at, grace_until) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (user_id) DO UPDATE SET \
             status = EXCLUDED.status, started_at = EXCLUDED.started_at, \
             expires_at = EXCLUDED.expires_at, grace_until = EXCLUDED.grace_until",
        )
        .bind(sub.id)
        .bind(sub.user_id)
        .bind(sub.status.as_str())
        .bind(sub.started_at)
        .bind(sub.expires_at)
        .bind(sub.grace_until)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn list_to_mark_grace(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Subscription>, String> {
        let rows: Vec<SubTuple> = sqlx::query_as(&format!(
            "SELECT {SUB_COLUMNS} FROM subscriptions \
             WHERE status = 'active' AND expires_at <= $1"
        ))
        .bind(now)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(sub_row).collect())
    }

    async fn list_to_mark_lapsed(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Subscription>, String> {
        let rows: Vec<SubTuple> = sqlx::query_as(&format!(
            "SELECT {SUB_COLUMNS} FROM subscriptions \
             WHERE status = 'grace' AND grace_until <= $1"
        ))
        .bind(now)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(sub_row).collect())
    }

    async fn list_to_delete(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Subscription>, String> {
        let rows: Vec<SubTuple> = sqlx::query_as(&format!(
            "SELECT {SUB_COLUMNS} FROM subscriptions \
             WHERE status IN ('lapsed', 'deleted') AND grace_until <= $1"
        ))
        .bind(before)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(sub_row).collect())
    }

    async fn set_status(
        &self,
        id: uuid::Uuid,
        status: SubscriptionStatus,
        grace_until: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), String> {
        sqlx::query("UPDATE subscriptions SET status = $2, grace_until = $3 WHERE id = $1")
            .bind(id)
            .bind(status.as_str())
            .bind(grace_until)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub struct PgPaymentOrderRepo {
    pub pool: PgPool,
}

type OrderTuple = (
    uuid::Uuid,
    uuid::Uuid,
    Decimal,
    String,
    String,
    String,
    Option<String>,
    chrono::DateTime<chrono::Utc>,
    Option<chrono::DateTime<chrono::Utc>>,
);

fn order_row(row: OrderTuple) -> PaymentOrder {
    let (id, user_id, amount, mint, recipient, status, signature, created_at, paid_at) = row;
    PaymentOrder {
        id,
        user_id,
        amount,
        mint,
        recipient,
        status: PaymentOrderStatus::parse(&status).unwrap_or(PaymentOrderStatus::Pending),
        signature,
        created_at,
        paid_at,
    }
}

const ORDER_COLUMNS: &str =
    "id, user_id, amount, mint, recipient, status, signature, created_at, paid_at";

#[async_trait]
impl PaymentOrderRepo for PgPaymentOrderRepo {
    async fn insert(&self, order: &PaymentOrder) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO payment_orders \
             (id, user_id, amount, mint, recipient, status, signature, created_at, paid_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(order.id)
        .bind(order.user_id)
        .bind(order.amount)
        .bind(&order.mint)
        .bind(&order.recipient)
        .bind(order.status.as_str())
        .bind(&order.signature)
        .bind(order.created_at)
        .bind(order.paid_at)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find_by_id(&self, id: uuid::Uuid) -> Result<Option<PaymentOrder>, String> {
        let row: Option<OrderTuple> = sqlx::query_as(&format!(
            "SELECT {ORDER_COLUMNS} FROM payment_orders WHERE id = $1"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(row.map(order_row))
    }

    async fn find_by_signature(&self, signature: &str) -> Result<Option<PaymentOrder>, String> {
        let row: Option<OrderTuple> = sqlx::query_as(&format!(
            "SELECT {ORDER_COLUMNS} FROM payment_orders WHERE signature = $1"
        ))
        .bind(signature)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(row.map(order_row))
    }

    async fn mark_paid_if_pending(
        &self,
        id: uuid::Uuid,
        signature: &str,
        paid_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<bool, String> {
        let res = sqlx::query(
            "UPDATE payment_orders SET status = 'paid', signature = $2, paid_at = $3 \
             WHERE id = $1 AND status = 'pending'",
        )
        .bind(id)
        .bind(signature)
        .bind(paid_at)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(res.rows_affected() == 1)
    }
}

pub struct PgStorageRepo {
    pub pool: PgPool,
}

const STORAGE_TOTAL_SQL: &str =
    "SELECT COALESCE(SUM(bytes), 0)::bigint FROM storage_files WHERE shop_id = $1";

#[async_trait]
impl StorageUsageRepo for PgStorageRepo {
    async fn add_checked(
        &self,
        shop_id: uuid::Uuid,
        key: uuid::Uuid,
        bytes: i64,
        quota: i64,
    ) -> Result<i64, StorageError> {
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        // Idempotent per key: re-recording the same file is a no-op.
        sqlx::query(
            "INSERT INTO storage_files (shop_id, key, bytes) VALUES ($1, $2, $3) \
             ON CONFLICT (shop_id, key) DO NOTHING",
        )
        .bind(shop_id)
        .bind(key)
        .bind(bytes)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        let total: (i64,) = sqlx::query_as(STORAGE_TOTAL_SQL)
            .bind(shop_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        if total.0 > quota {
            return Err(StorageError::QuotaExceeded);
        }
        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(total.0)
    }

    async fn total(&self, shop_id: uuid::Uuid) -> Result<i64, StorageError> {
        let total: (i64,) = sqlx::query_as(STORAGE_TOTAL_SQL)
            .bind(shop_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(total.0)
    }

    async fn list(&self, shop_id: uuid::Uuid) -> Result<Vec<(uuid::Uuid, i64)>, StorageError> {
        let rows: Vec<(uuid::Uuid, i64)> =
            sqlx::query_as("SELECT key, bytes FROM storage_files WHERE shop_id = $1 ORDER BY key")
                .bind(shop_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        Ok(rows)
    }

    async fn remove(&self, shop_id: uuid::Uuid, key: uuid::Uuid) -> Result<i64, StorageError> {
        sqlx::query("DELETE FROM storage_files WHERE shop_id = $1 AND key = $2")
            .bind(shop_id)
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        self.total(shop_id).await
    }
}
