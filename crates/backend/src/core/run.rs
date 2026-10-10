use crate::app::service::maintenance::run_maintenance;
use crate::app::service::use_cases::AppServices;
use pos_http::router::build_router;
use tracing_subscriber::EnvFilter;

pub const DEFAULT_ADDR: &str = "0.0.0.0:8080";
pub const FALLBACK_LOG: &str = "info";
const MAINTENANCE_INTERVAL_SECS: u64 = 300;

pub async fn run(services: AppServices) -> Result<(), std::io::Error> {
    init_tracing();
    spawn_maintenance_loop(services.clone());
    let app = build_router(services);
    let listener = tokio::net::TcpListener::bind(DEFAULT_ADDR).await?;
    tracing::info!(addr = DEFAULT_ADDR, "backend listening");
    axum::serve(listener, app).await
}

/// Expiry → grace → deletion sweep; idempotent, safe to repeat.
fn spawn_maintenance_loop(services: AppServices) {
    tokio::spawn(async move {
        let mut ticker =
            tokio::time::interval(std::time::Duration::from_secs(MAINTENANCE_INTERVAL_SECS));
        loop {
            ticker.tick().await;
            let cfg = services.payment_config.clone();
            match run_maintenance(&services, &cfg, chrono::Utc::now()).await {
                Ok(report)
                    if report.marked_grace + report.marked_lapsed + report.hard_deleted_shops
                        > 0 =>
                {
                    tracing::info!(?report, "maintenance sweep");
                }
                Ok(_) => {}
                Err(e) => tracing::error!(error = %e, "maintenance sweep failed"),
            }
        }
    });
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(FALLBACK_LOG));
    let _ = tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .try_init();
}
