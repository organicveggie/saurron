use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    Json,
    body::Bytes,
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tracing::{error, info};

use crate::{config, docker, metrics, notifications, registry, update};

pub struct AppStateInner {
    pub docker: docker::DockerClient,
    pub registry: registry::RegistryClient,
    pub config: config::Config,
    pub selector: docker::ContainerSelector,
    /// Held for the duration of any update cycle. Scheduler: .lock().await; HTTP: .try_lock().
    pub update_lock: tokio::sync::Mutex<()>,
    #[cfg(feature = "web")]
    pub pool: Option<sqlx::SqlitePool>,
}

pub type AppState = Arc<AppStateInner>;

#[derive(Debug, Deserialize)]
struct UpdateQuery {
    container: Option<String>,
    image: Option<String>,
}

/// Deserialises from a JSON string (optionally comma-separated) or a JSON/form
/// array into a `Vec<String>`. Values are split on commas and trimmed.
#[derive(Debug)]
struct StringOrVec(Vec<String>);

impl<'de> Deserialize<'de> for StringOrVec {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = StringOrVec;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a string or array of strings")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<StringOrVec, E> {
                Ok(StringOrVec(
                    v.split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect(),
                ))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<StringOrVec, A::Error> {
                let mut out = Vec::new();
                while let Some(s) = seq.next_element::<String>()? {
                    out.extend(
                        s.split(',')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(String::from),
                    );
                }
                Ok(StringOrVec(out))
            }
        }
        d.deserialize_any(Visitor)
    }
}

/// Optional request body for `POST /v1/update`.
#[derive(Debug, Default, Deserialize)]
struct UpdateBody {
    container: Option<StringOrVec>,
    image: Option<StringOrVec>,
}

/// Error returned by body-parsing and filter-merging helpers.
#[derive(Debug)]
enum UpdateParamError {
    BadRequest(String),
    UnsupportedMediaType,
}

impl IntoResponse for UpdateParamError {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": msg})),
            )
                .into_response(),
            Self::UnsupportedMediaType => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                [(
                    "Accept-Post",
                    "application/json, application/x-www-form-urlencoded",
                )],
            )
                .into_response(),
        }
    }
}

/// Container and image scope filters resolved from query params and/or body.
type UpdateFilters = (Option<Vec<String>>, Option<Vec<String>>);

/// Parse an optional request body as JSON or form-encoded.
///
/// Returns `Ok(None)` for an empty body, `Ok(Some(_))` on success,
/// or `Err` (400 or 415) on failure.
fn parse_update_body(
    content_type: Option<&str>,
    body: &[u8],
) -> Result<Option<UpdateBody>, UpdateParamError> {
    if body.is_empty() {
        return Ok(None);
    }
    let ct = content_type.unwrap_or("");
    if ct.starts_with("application/json") {
        serde_json::from_slice(body)
            .map(Some)
            .map_err(|e| UpdateParamError::BadRequest(format!("invalid JSON body: {e}")))
    } else if ct.starts_with("application/x-www-form-urlencoded") {
        serde_urlencoded::from_bytes(body)
            .map(Some)
            .map_err(|e| UpdateParamError::BadRequest(format!("invalid form body: {e}")))
    } else {
        Err(UpdateParamError::UnsupportedMediaType)
    }
}

/// Merge query params and an optional body into `(container_filter, image_filter)`.
///
/// Returns `Err` (400) if the same field is present in both.
fn resolve_update_filters(
    query: &UpdateQuery,
    body: Option<UpdateBody>,
) -> Result<UpdateFilters, UpdateParamError> {
    let body_container = body.as_ref().and_then(|b| b.container.as_ref());
    let body_image = body.as_ref().and_then(|b| b.image.as_ref());

    if query.container.is_some() && body_container.is_some() {
        return Err(UpdateParamError::BadRequest(
            "'container' specified in both query params and request body".into(),
        ));
    }
    if query.image.is_some() && body_image.is_some() {
        return Err(UpdateParamError::BadRequest(
            "'image' specified in both query params and request body".into(),
        ));
    }

    let container = query
        .container
        .as_deref()
        .map(|s| {
            s.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .or_else(|| body_container.map(|sv| sv.0.clone()));

    let image = query
        .image
        .as_deref()
        .map(|s| {
            s.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .or_else(|| body_image.map(|sv| sv.0.clone()));

    Ok((container, image))
}

/// Validate that the HTTP API token configuration is consistent.
/// Called at startup before binding the port.
pub fn validate_token_config(cfg: &config::HttpApiConfig) -> anyhow::Result<()> {
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
#[cfg_attr(not(feature = "web"), allow(unused_variables))]
pub async fn run_cycle_with_state(state: &AppStateInner, trigger: &str) {
    let all = match state.docker.list_containers(&state.selector).await {
        Ok(v) => v,
        Err(e) => {
            error!(error = %e, "failed to list containers for update cycle");
            return;
        }
    };
    let selected = state.docker.select_containers(&all, &state.selector);
    let report = update::UpdateEngine::new(&state.docker, &state.registry, &state.config)
        .run_cycle(&selected)
        .await;
    metrics::record_cycle(&report);
    notifications::dispatch(&state.config.notifications, &report).await;
    #[cfg(feature = "web")]
    if let Some(pool) = &state.pool {
        if let Err(e) = crate::db::record_cycle(pool, &report, trigger).await {
            error!(error = %e, trigger, "failed to persist cycle to database");
        }
    }
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"})))
}

async fn post_update(
    State(state): State<AppState>,
    Query(query): Query<UpdateQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Some(token) = &state.config.http_api.token
        && !check_auth(&headers, token)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    let body_params = match parse_update_body(content_type, &body) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };

    let (container_filter, image_filter) = match resolve_update_filters(&query, body_params) {
        Ok(f) => f,
        Err(e) => return e.into_response(),
    };

    let Ok(_guard) = state.update_lock.try_lock() else {
        metrics::record_skipped_cycle();
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

    if let Some(ref names) = container_filter {
        let allowed: std::collections::HashSet<&str> = names.iter().map(String::as_str).collect();
        selected.retain(|c| allowed.contains(c.name.as_str()));
    }
    if let Some(ref images) = image_filter {
        selected.retain(|c| images.iter().any(|img| c.image.starts_with(img.as_str())));
    }

    let report = update::UpdateEngine::new(&state.docker, &state.registry, &state.config)
        .run_cycle(&selected)
        .await;

    metrics::record_cycle(&report);
    notifications::dispatch(&state.config.notifications, &report).await;
    #[cfg(feature = "web")]
    if let Some(pool) = &state.pool {
        if let Err(e) = crate::db::record_cycle(pool, &report, "http_api").await {
            error!(error = %e, trigger = "http_api", "failed to persist cycle to database");
        }
    }

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

fn redact_authorization(value: &str) -> String {
    let mut parts = value.splitn(2, ' ');
    match (parts.next(), parts.next()) {
        (Some(scheme), Some(_)) => format!("{scheme} [REDACTED]"),
        _ => "[REDACTED]".to_string(),
    }
}

async fn access_log_middleware(
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let version = request.version();
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<not set>")
        .to_string();
    let user_agent = request
        .headers()
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<not set>")
        .to_string();
    let authorization = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(redact_authorization)
        .unwrap_or_else(|| "<not set>".to_string());

    let response = next.run(request).await;

    tracing::info!(
        target: "saurron::access",
        remote_addr = %remote_addr,
        host        = %host,
        uri         = %uri,
        path        = uri.path(),
        method      = %method,
        protocol    = ?version,
        scheme      = uri.scheme_str().unwrap_or("http"),
        user_agent  = %user_agent,
        authorization = %authorization,
        status      = response.status().as_u16(),
        duration_ms = start.elapsed().as_millis() as u64,
        "",
    );

    response
}

/// Builds the Axum router with all configured routes and middleware.
/// The caller is responsible for binding a listener and calling [`axum::serve`].
pub fn build_router(state: AppState) -> axum::Router {
    let has_access_log = state.config.http_api.access_log.is_some();
    let mut router = axum::Router::new().route("/v1/health", get(health));
    if state.config.http_api.update {
        router = router.route("/v1/update", post(post_update));
    }
    if state.config.http_api.metrics {
        router = router.route("/v1/metrics", get(get_metrics));
    }
    let router = router.with_state(state);
    if has_access_log {
        router.layer(axum::middleware::from_fn(access_log_middleware))
    } else {
        router
    }
}

pub async fn start_server(state: AppState) -> anyhow::Result<()> {
    use anyhow::Context as _;

    let port = state.config.http_api.port;
    let router = build_router(state);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind HTTP API port {port}"))?;
    info!(port, "HTTP API server listening");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
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
            access_log: None,
            #[cfg(feature = "web")]
            web_ui: false,
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

    #[test]
    fn redact_authorization_bearer() {
        assert_eq!(redact_authorization("Bearer mytoken"), "Bearer [REDACTED]");
    }

    #[test]
    fn redact_authorization_basic() {
        assert_eq!(
            redact_authorization("Basic dXNlcjpwYXNz"),
            "Basic [REDACTED]"
        );
    }

    #[test]
    fn redact_authorization_bare_value() {
        assert_eq!(redact_authorization("notype"), "[REDACTED]");
    }

    // ── StringOrVec ───────────────────────────────────────────────────────────

    fn parse_sov_json(s: &str) -> Vec<String> {
        serde_json::from_str::<StringOrVec>(s).unwrap().0
    }

    #[test]
    fn string_or_vec_single_string() {
        assert_eq!(parse_sov_json(r#""nginx""#), vec!["nginx"]);
    }

    #[test]
    fn string_or_vec_comma_separated_string() {
        assert_eq!(parse_sov_json(r#""nginx,redis""#), vec!["nginx", "redis"]);
    }

    #[test]
    fn string_or_vec_json_array() {
        assert_eq!(
            parse_sov_json(r#"["nginx","redis"]"#),
            vec!["nginx", "redis"]
        );
    }

    #[test]
    fn string_or_vec_trims_whitespace() {
        assert_eq!(
            parse_sov_json(r#"" nginx , redis ""#),
            vec!["nginx", "redis"]
        );
    }

    #[test]
    fn string_or_vec_via_form_encoding() {
        let body: UpdateBody = serde_urlencoded::from_str("container=nginx,redis").unwrap();
        assert_eq!(body.container.unwrap().0, vec!["nginx", "redis"]);
    }

    // ── parse_update_body ─────────────────────────────────────────────────────

    #[test]
    fn parse_body_empty_is_none() {
        assert!(parse_update_body(None, b"").unwrap().is_none());
    }

    #[test]
    fn parse_body_json_string_fields() {
        let b = parse_update_body(
            Some("application/json"),
            br#"{"container":"nginx","image":"nginx:latest"}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(b.container.unwrap().0, vec!["nginx"]);
        assert_eq!(b.image.unwrap().0, vec!["nginx:latest"]);
    }

    #[test]
    fn parse_body_json_array_fields() {
        let b = parse_update_body(
            Some("application/json"),
            br#"{"container":["nginx","redis"]}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(b.container.unwrap().0, vec!["nginx", "redis"]);
    }

    #[test]
    fn parse_body_form_encoded() {
        let b = parse_update_body(
            Some("application/x-www-form-urlencoded"),
            b"container=nginx,redis",
        )
        .unwrap()
        .unwrap();
        assert_eq!(b.container.unwrap().0, vec!["nginx", "redis"]);
    }

    #[test]
    fn parse_body_wrong_content_type_returns_415() {
        assert!(matches!(
            parse_update_body(Some("text/plain"), b"container=nginx").unwrap_err(),
            UpdateParamError::UnsupportedMediaType
        ));
    }

    #[test]
    fn parse_body_absent_content_type_with_body_returns_415() {
        assert!(matches!(
            parse_update_body(None, b"container=nginx").unwrap_err(),
            UpdateParamError::UnsupportedMediaType
        ));
    }

    #[test]
    fn unsupported_media_type_response_includes_accept_post_header() {
        let resp = UpdateParamError::UnsupportedMediaType.into_response();
        assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(
            resp.headers()
                .get("Accept-Post")
                .and_then(|v| v.to_str().ok()),
            Some("application/json, application/x-www-form-urlencoded"),
        );
    }

    #[test]
    fn parse_body_malformed_json_returns_400() {
        assert!(matches!(
            parse_update_body(Some("application/json"), b"{bad json}").unwrap_err(),
            UpdateParamError::BadRequest(_)
        ));
    }

    // ── resolve_update_filters ────────────────────────────────────────────────

    fn query(container: Option<&str>, image: Option<&str>) -> UpdateQuery {
        UpdateQuery {
            container: container.map(String::from),
            image: image.map(String::from),
        }
    }

    fn body_with(container: Option<&str>, image: Option<&str>) -> Option<UpdateBody> {
        Some(UpdateBody {
            container: container.map(|s| StringOrVec(vec![s.to_string()])),
            image: image.map(|s| StringOrVec(vec![s.to_string()])),
        })
    }

    #[test]
    fn resolve_query_only() {
        let (c, i) = resolve_update_filters(&query(Some("nginx,redis"), None), None).unwrap();
        assert_eq!(c.unwrap(), vec!["nginx", "redis"]);
        assert!(i.is_none());
    }

    #[test]
    fn resolve_body_only() {
        let (c, i) =
            resolve_update_filters(&query(None, None), body_with(Some("nginx"), None)).unwrap();
        assert_eq!(c.unwrap(), vec!["nginx"]);
        assert!(i.is_none());
    }

    #[test]
    fn resolve_no_overlap_different_fields() {
        let (c, i) = resolve_update_filters(
            &query(Some("nginx"), None),
            body_with(None, Some("nginx:latest")),
        )
        .unwrap();
        assert_eq!(c.unwrap(), vec!["nginx"]);
        assert_eq!(i.unwrap(), vec!["nginx:latest"]);
    }

    #[test]
    fn resolve_conflict_container_returns_400() {
        assert!(matches!(
            resolve_update_filters(&query(Some("nginx"), None), body_with(Some("nginx"), None),)
                .unwrap_err(),
            UpdateParamError::BadRequest(_)
        ));
    }

    #[test]
    fn resolve_conflict_image_returns_400() {
        assert!(matches!(
            resolve_update_filters(
                &query(None, Some("nginx:latest")),
                body_with(None, Some("nginx:latest")),
            )
            .unwrap_err(),
            UpdateParamError::BadRequest(_)
        ));
    }

    #[test]
    fn resolve_no_params_returns_none_filters() {
        let (c, i) = resolve_update_filters(&query(None, None), None).unwrap();
        assert!(c.is_none());
        assert!(i.is_none());
    }
}
