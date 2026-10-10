//! Value objects: validated at construction, immutable.

use rust_decimal::Decimal;
use thiserror::Error;

pub const SOL_MAX: usize = 44;
pub const ETH_LEN: usize = 42;
pub const BTC_MAX: usize = 62;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("invalid email")]
    Email,
    #[error("invalid wallet address")]
    Wallet,

    #[error("invalid chain address")]
    Address,
    #[error("amount must be positive")]
    Amount,
    #[error("name must not be empty")]
    Name,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Employee,
    Admin,
    Owner,
    SuperAdmin,
}

impl Role {
    #[must_use]
    pub fn can_manage(self, other: Self) -> bool {
        self > other
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Employee => "employee",
            Self::Admin => "admin",
            Self::Owner => "owner",
            Self::SuperAdmin => "super_admin",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "employee" => Some(Self::Employee),
            "admin" => Some(Self::Admin),
            "owner" => Some(Self::Owner),
            "super_admin" => Some(Self::SuperAdmin),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email(String);

impl Email {
    /// Minimal shape check: one `@`, non-empty local part, domain with a dot.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let (local, domain) = raw.split_once('@').ok_or(ValidationError::Email)?;
        let valid = !local.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
            && raw.is_ascii()
            && !raw.contains(char::is_whitespace);
        if valid {
            Ok(Self(raw.to_ascii_lowercase()))
        } else {
            Err(ValidationError::Email)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Bypasses validation; caller guarantees the value came from a trusted store.
    #[must_use]
    pub fn unchecked(raw: String) -> Self {
        Self(raw)
    }
}

/// Solana wallet public key (base58, ed25519 pubkey length range).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wallet(String);

impl Wallet {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let ok = (SOL_MIN..=SOL_MAX).contains(&raw.len())
            && raw
                .chars()
                .all(|c| c.is_ascii_alphanumeric() && !matches!(c, '0' | 'O' | 'I' | 'l'));
        if ok {
            Ok(Self(raw.to_owned()))
        } else {
            Err(ValidationError::Wallet)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Bypasses validation; caller guarantees the value came from a trusted store.
    #[must_use]
    pub fn unchecked(raw: String) -> Self {
        Self(raw)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Money(Decimal);

impl Money {
    pub fn new(amount: Decimal) -> Result<Self, ValidationError> {
        if amount.is_sign_negative() || amount.is_zero() {
            Err(ValidationError::Amount)
        } else {
            Ok(Self(amount))
        }
    }

    pub fn amount(&self) -> Decimal {
        self.0
    }

    #[must_use]
    pub const fn one() -> Self {
        Self(Decimal::ONE)
    }

    /// Sum amounts; `None` only if the iterator is empty.
    pub fn total<'a>(mut items: impl Iterator<Item = &'a Self>) -> Option<Self> {
        items
            .next()
            .map(|first| Self(items.fold(first.0, |acc, m| acc + m.0)))
    }
}

pub const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
pub const USDT_MINT: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplToken {
    #[default]
    Sol,
    Usdc,
    Usdt,
}

impl SplToken {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sol => "sol",
            Self::Usdc => "usdc",
            Self::Usdt => "usdt",
        }
    }

    #[must_use]
    pub const fn mint(self) -> Option<&'static str> {
        match self {
            Self::Sol => None,
            Self::Usdc => Some(USDC_MINT),
            Self::Usdt => Some(USDT_MINT),
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "sol" => Some(Self::Sol),
            "usdc" => Some(Self::Usdc),
            "usdt" => Some(Self::Usdt),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    Solana,
    Eth,
    Btc,
    Binance,
    Bank,
}

impl Chain {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Solana => "solana",
            Self::Eth => "eth",
            Self::Btc => "btc",
            Self::Binance => "binance",
            Self::Bank => "bank",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "solana" => Some(Self::Solana),
            "eth" => Some(Self::Eth),
            "btc" => Some(Self::Btc),
            "binance" => Some(Self::Binance),
            "bank" => Some(Self::Bank),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainAddress {
    chain: Chain,
    value: String,
}

impl ChainAddress {
    fn is_base58(raw: &str) -> bool {
        raw.chars()
            .all(|c| c.is_ascii_alphanumeric() && !matches!(c, '0' | 'O' | 'I' | 'l'))
    }

    fn is_hex(raw: &str) -> bool {
        raw.len() > 2 && raw[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    pub fn parse(chain: Chain, raw: &str) -> Result<Self, ValidationError> {
        let ok = match chain {
            Chain::Solana => {
                let len = raw.len();
                (SOL_MIN..=SOL_MAX).contains(&len) && Self::is_base58(raw)
            }
            Chain::Eth => raw.len() == ETH_LEN && raw.starts_with("0x") && Self::is_hex(raw),
            Chain::Btc => {
                let len = raw.len();
                let bech32 =
                    raw.starts_with("bc1") && raw.chars().all(|c| c.is_ascii_alphanumeric());
                let base58 = Self::is_base58(raw) && (raw.starts_with('1') || raw.starts_with('3'));
                (bech32 || base58) && len <= BTC_MAX
            }
            Chain::Binance => !raw.is_empty() && raw.chars().all(|c| c.is_ascii_alphanumeric()),
            Chain::Bank => {
                let len = raw.len();
                (BANK_MIN..=BANK_MAX).contains(&len)
                    && raw
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | ' '))
            }
        };
        if ok {
            Ok(Self {
                chain,
                value: raw.to_owned(),
            })
        } else {
            Err(ValidationError::Address)
        }
    }

    pub fn chain(&self) -> Chain {
        self.chain
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Bypasses validation; caller guarantees the value came from a trusted store.
    #[must_use]
    pub fn unchecked(chain: Chain, value: String) -> Self {
        Self { chain, value }
    }
}

const SOL_MIN: usize = 32;
const BANK_MIN: usize = 6;
const BANK_MAX: usize = 34;

pub fn new_id() -> uuid::Uuid {
    uuid::Uuid::now_v7()
}

/// SVG icon key shown on goods tiles; rendered by the UI, never emoji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GoodsIcon {
    #[default]
    Package,
    Cart,
    Coffee,
    Burger,
    Pizza,
    Soda,
    Water,
    Beer,
    Bread,
    Cake,
    Rice,
    Noodle,
    Apple,
    Carrot,
    Meat,
    Fish,
    Milk,
    Egg,
    Candy,
    IceCream,
    Shirt,
    Hat,
    Shoe,
    Bag,
    Gift,
    Phone,
    Laptop,
    Headphone,
    Camera,
    Book,
    Pen,
    Flower,
    Dog,
    Cat,
    Pill,
    Soap,
    Gas,
    Ticket,
    Wrench,
    Star,
}

pub const GOODS_ICON_COUNT: usize = 40;

impl GoodsIcon {
    pub const ALL: [Self; GOODS_ICON_COUNT] = [
        Self::Package,
        Self::Cart,
        Self::Coffee,
        Self::Burger,
        Self::Pizza,
        Self::Soda,
        Self::Water,
        Self::Beer,
        Self::Bread,
        Self::Cake,
        Self::Rice,
        Self::Noodle,
        Self::Apple,
        Self::Carrot,
        Self::Meat,
        Self::Fish,
        Self::Milk,
        Self::Egg,
        Self::Candy,
        Self::IceCream,
        Self::Shirt,
        Self::Hat,
        Self::Shoe,
        Self::Bag,
        Self::Gift,
        Self::Phone,
        Self::Laptop,
        Self::Headphone,
        Self::Camera,
        Self::Book,
        Self::Pen,
        Self::Flower,
        Self::Dog,
        Self::Cat,
        Self::Pill,
        Self::Soap,
        Self::Gas,
        Self::Ticket,
        Self::Wrench,
        Self::Star,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Package => "package",
            Self::Cart => "cart",
            Self::Coffee => "coffee",
            Self::Burger => "burger",
            Self::Pizza => "pizza",
            Self::Soda => "soda",
            Self::Water => "water",
            Self::Beer => "beer",
            Self::Bread => "bread",
            Self::Cake => "cake",
            Self::Rice => "rice",
            Self::Noodle => "noodle",
            Self::Apple => "apple",
            Self::Carrot => "carrot",
            Self::Meat => "meat",
            Self::Fish => "fish",
            Self::Milk => "milk",
            Self::Egg => "egg",
            Self::Candy => "candy",
            Self::IceCream => "icecream",
            Self::Shirt => "shirt",
            Self::Hat => "hat",
            Self::Shoe => "shoe",
            Self::Bag => "bag",
            Self::Gift => "gift",
            Self::Phone => "phone",
            Self::Laptop => "laptop",
            Self::Headphone => "headphone",
            Self::Camera => "camera",
            Self::Book => "book",
            Self::Pen => "pen",
            Self::Flower => "flower",
            Self::Dog => "dog",
            Self::Cat => "cat",
            Self::Pill => "pill",
            Self::Soap => "soap",
            Self::Gas => "gas",
            Self::Ticket => "ticket",
            Self::Wrench => "wrench",
            Self::Star => "star",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|i| i.as_str() == raw)
    }
}

/// Product category picked from a dropdown in the goods form; stored as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GoodsType {
    Service,
    Food,
    Drink,
    Dessert,
    Electronic,
    Clothing,
    Cosmetic,
    Ticket,
    Prop,
    #[default]
    Other,
}

pub const GOODS_TYPE_COUNT: usize = 10;

impl GoodsType {
    pub const ALL: [Self; GOODS_TYPE_COUNT] = [
        Self::Service,
        Self::Food,
        Self::Drink,
        Self::Dessert,
        Self::Electronic,
        Self::Clothing,
        Self::Cosmetic,
        Self::Ticket,
        Self::Prop,
        Self::Other,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Service => "service",
            Self::Food => "food",
            Self::Drink => "drink",
            Self::Dessert => "dessert",
            Self::Electronic => "electronic",
            Self::Clothing => "clothing",
            Self::Cosmetic => "cosmetic",
            Self::Ticket => "ticket",
            Self::Prop => "prop",
            Self::Other => "other",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|t| t.as_str() == raw)
    }
}

/// Pastel tile background color, keyed `c01`..`c40` (see .plans/28).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PastelColor {
    #[default]
    C01,
    C02,
    C03,
    C04,
    C05,
    C06,
    C07,
    C08,
    C09,
    C10,
    C11,
    C12,
    C13,
    C14,
    C15,
    C16,
    C17,
    C18,
    C19,
    C20,
    C21,
    C22,
    C23,
    C24,
    C25,
    C26,
    C27,
    C28,
    C29,
    C30,
    C31,
    C32,
    C33,
    C34,
    C35,
    C36,
    C37,
    C38,
    C39,
    C40,
}

pub const PASTEL_COLOR_COUNT: usize = 40;

impl PastelColor {
    pub const ALL: [Self; PASTEL_COLOR_COUNT] = [
        Self::C01,
        Self::C02,
        Self::C03,
        Self::C04,
        Self::C05,
        Self::C06,
        Self::C07,
        Self::C08,
        Self::C09,
        Self::C10,
        Self::C11,
        Self::C12,
        Self::C13,
        Self::C14,
        Self::C15,
        Self::C16,
        Self::C17,
        Self::C18,
        Self::C19,
        Self::C20,
        Self::C21,
        Self::C22,
        Self::C23,
        Self::C24,
        Self::C25,
        Self::C26,
        Self::C27,
        Self::C28,
        Self::C29,
        Self::C30,
        Self::C31,
        Self::C32,
        Self::C33,
        Self::C34,
        Self::C35,
        Self::C36,
        Self::C37,
        Self::C38,
        Self::C39,
        Self::C40,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::C01 => "c01",
            Self::C02 => "c02",
            Self::C03 => "c03",
            Self::C04 => "c04",
            Self::C05 => "c05",
            Self::C06 => "c06",
            Self::C07 => "c07",
            Self::C08 => "c08",
            Self::C09 => "c09",
            Self::C10 => "c10",
            Self::C11 => "c11",
            Self::C12 => "c12",
            Self::C13 => "c13",
            Self::C14 => "c14",
            Self::C15 => "c15",
            Self::C16 => "c16",
            Self::C17 => "c17",
            Self::C18 => "c18",
            Self::C19 => "c19",
            Self::C20 => "c20",
            Self::C21 => "c21",
            Self::C22 => "c22",
            Self::C23 => "c23",
            Self::C24 => "c24",
            Self::C25 => "c25",
            Self::C26 => "c26",
            Self::C27 => "c27",
            Self::C28 => "c28",
            Self::C29 => "c29",
            Self::C30 => "c30",
            Self::C31 => "c31",
            Self::C32 => "c32",
            Self::C33 => "c33",
            Self::C34 => "c34",
            Self::C35 => "c35",
            Self::C36 => "c36",
            Self::C37 => "c37",
            Self::C38 => "c38",
            Self::C39 => "c39",
            Self::C40 => "c40",
        }
    }

    #[must_use]
    pub const fn hex(self) -> &'static str {
        match self {
            Self::C01 => "#FFADAD",
            Self::C02 => "#FFD6A5",
            Self::C03 => "#FDFFB6",
            Self::C04 => "#CAFFBF",
            Self::C05 => "#9BF6FF",
            Self::C06 => "#A0C4FF",
            Self::C07 => "#BDB2FF",
            Self::C08 => "#FFC6FF",
            Self::C09 => "#FDE4CF",
            Self::C10 => "#FBF8CC",
            Self::C11 => "#FFC9B9",
            Self::C12 => "#FFB5A7",
            Self::C13 => "#F8EDEB",
            Self::C14 => "#FCD5CE",
            Self::C15 => "#FAE1DD",
            Self::C16 => "#D8E2DC",
            Self::C17 => "#E8E8E4",
            Self::C18 => "#ECEAE4",
            Self::C19 => "#CFE1B9",
            Self::C20 => "#E9EDC9",
            Self::C21 => "#CCD5AE",
            Self::C22 => "#E9F5DB",
            Self::C23 => "#FFE5EC",
            Self::C24 => "#FFC8DD",
            Self::C25 => "#FFB7B2",
            Self::C26 => "#FFDAC1",
            Self::C27 => "#E2F0CB",
            Self::C28 => "#B5EAD7",
            Self::C29 => "#C7CEEA",
            Self::C30 => "#F7D6E0",
            Self::C31 => "#D0F4DE",
            Self::C32 => "#A9DEF9",
            Self::C33 => "#E4C1F9",
            Self::C34 => "#FFD9C0",
            Self::C35 => "#FFF3B0",
            Self::C36 => "#D6E6F2",
            Self::C37 => "#EDE7FF",
            Self::C38 => "#FFE0AC",
            Self::C39 => "#C9E4C5",
            Self::C40 => "#F6C6EA",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.as_str() == raw)
    }
}
