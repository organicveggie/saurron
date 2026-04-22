use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tracing::{error, info};

use crate::{config, docker, registry, update};

pub(crate) struct AppStateInner {
    pub(crate) docker: docker::DockerClient,
    pub(crate) registry: registry::RegistryClient,
    pub(crate) config: config::Config,
    pub(crate) selector: docker::ContainerSelector,
    /// Held for the duration of any update cycle. Scheduler: .lock().await; HTTP: .try_lock().
    pub(crate) update_lock: tokio::sync::Mutex<()>,
}

pub(crate) type AppState = Arc<AppStateInner>;

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateQuery {
    pub(crate) container: Option<String>,
    pub(crate) image: Option<String>,
}

/// Validate that the HTTP API token configuration is consistent.
/// Called at startup before binding the port.
pub(crate) fn validate_token_config(cfg: &config::HttpApiConfig) -> anyhow::Result<()> {
    if cfg.update && cfg.token.is_none() {
        anyhow::bail!("--http-api-update requires --http-api-token");
    }
    if cfg.metrics && !cfg.metrics_no_auth && cfg.token.is_none() {
        anyhow::bail!(
            "--http-api-metrics requires --http-api-token (or --http-api-metrics-no-auth)"
        );
    }
    Ok(())
}

fn check_auth(headers: &HeaderMap, token: &str) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|provided| provided == token)
        .unwrap_or(false)
}

/// Run a full enumeration + update cycle using the shared application state.
pub(crate) async fn run_cycle_with_state(state: &AppStateInner) {
    let all = match state.docker.list_containers(&state.selector).await {
        Ok(v) => v,
        Err(e) => {
            error!(error = %e, "failed to list containers for update cycle");
            return;
        }
    };
    let selected = state.docker.select_containers(&all, &state.selector);
    update::UpdateEngine::new(&state.docker, &state.registry, &state.config)
        .run_cycle(&selected)
        .await;
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn post_update(
    State(state): State<AppState>,
    Query(query): Query<UpdateQuery>,
    headers: HeaderMap,
) -> Response {
    if let Some(token) = &state.config.http_api.token
        && !check_auth(&headers, token)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let Ok(_guard) = state.update_lock.try_lock() else {
        return StatusCode::CONFLICT.into_response();
    };

    let all = match state.docker.list_containers(&state.selector).await {
        Ok(v) => v,
        Err(e) => {
            error!(error = %e, "failed to list containers for HTTP-triggered update");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let mut selected = state.docker.select_containers(&all, &state.selector);

    if let Some(ref names) = query.container {
        let allowed: std::collections::HashSet<&str> = names.split(',').map(str::trim).collect();
        selected.retain(|c| allowed.contains(c.name.as_str()));
    }
    if let Some(ref image_filter) = query.image {
        let images: Vec<&str> = image_filter.split(',').map(str::trim).collect();
        selected.retain(|c| images.iter().any(|img| c.image.starts_with(img)));
    }

    let report = update::UpdateEngine::new(&state.docker, &state.registry, &state.config)
        .run_cycle(&selected)
        .await;

    Json(report).into_response()
}

async fn get_metrics(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !state.config.http_api.metrics_no_auth
        && let Some(token) = &state.config.http_api.token
        && !check_auth(&headers, token)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    use prometheus::Encoder as _;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        error!(error = %e, "failed to encode prometheus metrics");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let body = String::from_utf8_lossy(&buffer).into_owned();
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
        .into_response()
}

pub(crate) async fn start_server(state: AppState) -> anyhow::Result<()> {
    use anyhow::Context as _;

    let mut router = axum::Router::new().route("/v1/health", get(health));
    if state.config.http_api.update {
        router = router.route("/v1/update", post(post_update));
    }
    if state.config.http_api.metrics {
        router = router.route("/v1/metrics", get(get_metrics));
    }
    let router = router.with_state(state.clone());

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], state.config.http_api.port));
    let listener = tokio::net::TcpListener::bind(addr).await.with_context(|| {
        format!(
            "failed to bind HTTP API port {}",
            state.config.http_api.port
        )
    })?;
    info!(
        port = state.config.http_api.port,
        "HTTP API server listening"
    );
    axum::serve(listener, router)
        .await
        .context("HTTP API server error")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_http_config(
        update: bool,
        metrics: bool,
        token: Option<&str>,
        metrics_no_auth: bool,
    ) -> config::HttpApiConfig {
        config::HttpApiConfig {
            update,
            metrics,
            token: token.map(|s| s.to_string()),
            port: 8080,
            metrics_no_auth,
        }
    }

    #[test]
    fn validate_update_without_token_is_error() {
        let cfg = make_http_config(true, false, None, false);
        assert!(validate_token_config(&cfg).is_err());
    }

    #[test]
    fn validate_update_with_token_is_ok() {
        let cfg = make_http_config(true, false, Some("secret"), false);
        assert!(validate_token_config(&cfg).is_ok());
    }

    #[test]
    fn validate_metrics_no_auth_needs_no_token() {
        let cfg = make_http_config(false, true, None, true);
        assert!(validate_token_config(&cfg).is_ok());
    }

    #[test]
    fn validate_metrics_with_auth_needs_token() {
        let cfg = make_http_config(false, true, None, false);
        assert!(validate_token_config(&cfg).is_err());
    }

    #[test]
    fn validate_no_endpoints_no_token_needed() {
        let cfg = make_http_config(false, false, None, false);
        assert!(validate_token_config(&cfg).is_ok());
    }

    #[test]
    fn check_auth_valid_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer mysecret".parse().unwrap());
        assert!(check_auth(&headers, "mysecret"));
    }

    #[test]
    fn check_auth_wrong_token_fails() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer wrongtoken".parse().unwrap());
        assert!(!check_auth(&headers, "mysecret"));
    }

    #[test]
    fn check_auth_no_header_fails() {
        let headers = HeaderMap::new();
        assert!(!check_auth(&headers, "mysecret"));
    }

    #[test]
    fn check_auth_basic_auth_prefix_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Basic dXNlcjpwYXNz".parse().unwrap());
        assert!(!check_auth(&headers, "dXNlcjpwYXNz"));
    }
}
