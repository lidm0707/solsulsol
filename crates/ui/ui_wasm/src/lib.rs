//! Dioxus wasm app root.

pub mod api_client;
pub mod ui_interface_app;

pub use ui_interface_app::app_root;

/// Mount the app; called from the wasm bin entry.
pub fn app_launch() {
    console_error_panic_hook::set_once();
    dioxus_web::launch::launch_cfg(app_root, Default::default());
}
