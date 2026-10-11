//! Application services: orchestrate ports; combine repo data here (repos never JOIN).

pub mod auth;
pub mod billing;
pub mod destination;
pub mod goods;
pub mod maintenance;
pub mod shop;
pub mod subscription;
pub mod use_cases;

#[cfg(not(target_arch = "wasm32"))]
pub mod pdf;
