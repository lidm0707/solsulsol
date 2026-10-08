//! Backend mono-crate: layers as modules, deps point inward.
//! core -> app -> domain / port <- infra
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod core;
pub mod infra;
pub use pos_app as app;
pub use pos_app::domain;
pub use pos_app::port;
pub use pos_http as http_adapter;
