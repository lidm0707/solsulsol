use crate::app::service::use_cases::AppServices;
use crate::infra::http_adapter::router::build_router;
use tracing_subscriber::EnvFilter;

pub const DEFAULT_ADDR: &str = "0.0.0.0:8080";
pub const FALLBACK_LOG: &str = "info";

pub async fn run(services: AppServices) -> Result<(), std::io::Error> {
    init_tracing();
    let app = build_router(services);
    let listener = tokio::net::TcpListener::bind(DEFAULT_ADDR).await?;
    tracing::info!(addr = DEFAULT_ADDR, "backend listening");
    axum::serve(listener, app).await
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(FALLBACK_LOG));
    let _ = tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .try_init();
}
