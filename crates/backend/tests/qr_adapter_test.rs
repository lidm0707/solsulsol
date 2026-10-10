//! QrAdapter tests: implements QrRenderer over pos-qr-code.

use pos_backend::infra::qr_adapter::adapter::QrAdapter;
use pos_backend::port::qr::QrRenderer;

const PAYLOAD: &str = "pay:btc:bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4:01890a5d";

#[tokio::test]
async fn adapter_renders_png() {
    let png = QrAdapter
        .render_png(PAYLOAD)
        .await
        .expect("payload renders");
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']), "PNG magic");
}

#[tokio::test]
async fn adapter_propagates_encode_errors() {
    assert!(QrAdapter.render_png("").await.is_err());
}

#[tokio::test]
async fn adapter_deterministic_output() {
    let a = QrAdapter.render_png(PAYLOAD).await.expect("renders");
    let b = QrAdapter.render_png(PAYLOAD).await.expect("renders");
    assert_eq!(a, b, "same payload yields identical bytes");
}
