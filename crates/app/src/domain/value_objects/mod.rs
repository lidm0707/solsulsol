//! Value objects: validated at construction, immutable.

use rust_decimal::Decimal;
use thiserror::Error;

pub const MIN_PASSWORD_LEN: usize = 8;
pub const SOL_MAX: usize = 44;
pub const ETH_LEN: usize = 42;
pub const BTC_MAX: usize = 62;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("invalid email")]
    Email,
    #[error("password too short, min {MIN_PASSWORD_LEN}")]
    Password,
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

#[derive(Debug, Clone)]
pub struct PasswordHash(String);

impl PasswordHash {
    pub fn from_hash(hash: String) -> Self {
        Self(hash)
    }

    pub fn as_str(&self) -> &str {
        &self.0
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
