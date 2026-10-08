//! Backend mono-crate: layers as modules, deps point inward.
//! core -> app -> domain / port <- infra
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod app;
pub mod core;
pub mod domain;
pub mod infra;
pub mod port;
