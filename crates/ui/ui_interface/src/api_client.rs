//! Thin HTTP client placeholder; wasm impl uses gloo/fetch in ui_wasm.

#[derive(Default)]
pub struct HttpApiClient {
    pub base_url: String,
}
