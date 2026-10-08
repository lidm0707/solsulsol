//! Ports: async traits implemented by infra adapters, mocked in tests.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod database;
pub mod encryption;
pub mod http;
pub mod qr;

pub use database::{InvoiceRepo, ProductRepo, ShopMemberRepo, ShopRepo, UserRepo};
pub use encryption::{PasswordHasher, TokenIssuer};
pub use qr::QrRenderer;
