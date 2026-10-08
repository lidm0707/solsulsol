use pos_http::router::build_router;
use tower_service::Service as _;
use worker::*;

pub mod infra;

const API_PREFIX: &str = "/api";

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    let services = infra::wire_services(&env)?;
    Ok(build_router(services).call(strip_api_prefix(req)?).await?)
}

/// The assets config routes `/api/*` to this handler; the prefix is removed so
/// the shared axum router keeps its original paths (`/api/health` -> `/health`).
fn strip_api_prefix(req: HttpRequest) -> Result<HttpRequest> {
    let (mut parts, body) = req.into_parts();
    let path = parts.uri.path().to_string();
    let Some(stripped) = path.strip_prefix(API_PREFIX) else {
        return Ok(axum::http::Request::from_parts(parts, body));
    };
    let path_and_query = match parts.uri.query() {
        Some(q) => format!("{stripped}?{q}"),
        None => stripped.to_string(),
    };
    let uri = axum::http::Uri::builder()
        .path_and_query(path_and_query)
        .build()
        .map_err(|e| Error::RustError(e.to_string()))?;
    parts.uri = uri;
    Ok(axum::http::Request::from_parts(parts, body))
}
