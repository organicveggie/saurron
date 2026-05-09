use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::header;
use semver::Version;
use serde::Deserialize;
use thiserror::Error;
use tracing::{debug, warn};

use crate::cli::HeadWarnStrategy;

// ── Error types ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("authentication failed for {registry}: {reason}")]
    AuthFailed { registry: String, reason: String },
    #[error("manifest not found: {0}")]
    ManifestNotFound(String),
    #[error("unexpected registry response: {0}")]
    UnexpectedResponse(String),
    #[error("Docker Hub rate limited")]
    RateLimited { retry_after: Option<u64> },
}

// ── Image reference ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ImageRef {
    pub registry: String,
    pub repository: String,
    pub reference: ImageReference,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImageReference {
    Tag(String),
    Digest(String),
}

// ── Non-semver strategy ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NonSemverStrategy {
    #[default]
    Digest,
    Skip,
}

pub fn parse_non_semver_strategy(s: &str) -> NonSemverStrategy {
    match s.trim().to_ascii_lowercase().as_str() {
        "skip" => NonSemverStrategy::Skip,
        _ => NonSemverStrategy::Digest,
    }
}

// ── Freshness result ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum FreshnessResult {
    UpToDate,
    Stale(StaleInfo),
    Skipped(String),
    Error(String),
}

#[derive(Debug)]
pub struct StaleInfo {
    pub current_digest: String,
    /// Fully-qualified new image reference (`registry/repo:tag`).
    pub new_image: String,
    /// Manifest digest of the new image, or the new tag string for SemVer updates
    /// (actual digest fetched during pull in Phase 5).
    pub new_digest: String,
}

// ── Registry client ───────────────────────────────────────────────────────────

/// Returns `"http"` for localhost and any bare-IP registries, `"https"` otherwise.
/// Bare-IP registries (e.g. `172.17.0.1:5000`) are almost universally plain-HTTP
/// (Docker itself requires explicit `insecure-registries` config for TLS at an IP),
/// so treating them as http matches real-world usage and avoids TLS failures in
/// devcontainer / Docker-in-Docker test environments.
fn scheme_for_registry(registry: &str) -> &'static str {
    let host = registry.split(':').next().unwrap_or(registry);
    if host == "localhost" || host.parse::<std::net::IpAddr>().is_ok() {
        "http"
    } else {
        "https"
    }
}

pub struct RegistryClient {
    client: reqwest::Client,
    head_warn_strategy: HeadWarnStrategy,
    user_agent: String,
    /// Global fallback credentials applied when no per-registry entry matches.
    credentials: Option<(String, String)>,
    /// Per-registry overrides keyed by normalised hostname.
    /// `Some((u, p))` → use those credentials.
    /// `None` (key present, value `None`) → explicit anonymous for this registry.
    per_registry: HashMap<String, Option<(String, String)>>,
}

/// Normalise a registry hostname so both user-facing aliases and internal
/// hostnames resolve to the same key. Currently maps `"docker.io"` →
/// `"registry-1.docker.io"`; all other values are returned unchanged.
pub fn normalize_registry_host(host: &str) -> String {
    if host == "docker.io" {
        "registry-1.docker.io".to_string()
    } else {
        host.to_string()
    }
}

impl RegistryClient {
    pub fn new(
        head_warn_strategy: HeadWarnStrategy,
        version: &str,
        credentials: Option<(String, String)>,
        per_registry_entries: Vec<(String, Option<(String, String)>)>,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("failed to build registry HTTP client")?;
        let per_registry = per_registry_entries
            .into_iter()
            .map(|(host, creds)| (normalize_registry_host(&host), creds))
            .collect();
        Ok(Self {
            client,
            head_warn_strategy,
            user_agent: format!("saurron/{version}"),
            credentials,
            per_registry,
        })
    }

    /// Resolve credentials for a registry hostname.
    ///
    /// Lookup order:
    /// 1. Per-registry entry — `Some(creds)` to use those, `None` for explicit anonymous.
    /// 2. Global credentials (`self.credentials`).
    /// 3. Anonymous (if no global credentials).
    fn credentials_for(&self, registry: &str) -> Option<(String, String)> {
        if let Some(entry) = self.per_registry.get(registry) {
            entry.clone()
        } else {
            self.credentials.clone()
        }
    }

    /// Resolve credentials for the registry that hosts `image`.
    ///
    /// Parses the image reference to extract the registry hostname, then
    /// delegates to [`credentials_for`].
    pub(crate) fn credentials_for_image(&self, image: &str) -> Option<(String, String)> {
        let registry = parse_image_ref(image)
            .map(|r| r.registry)
            .unwrap_or_default();
        self.credentials_for(&registry)
    }

    pub async fn check_freshness(
        &self,
        image: &str,
        local_digest: Option<&str>,
        allow_prerelease: bool,
        non_semver_strategy: NonSemverStrategy,
    ) -> FreshnessResult {
        let image_ref = match parse_image_ref(image) {
            Ok(r) => r,
            Err(e) => {
                return FreshnessResult::Error(format!("failed to parse image ref '{image}': {e}"));
            }
        };
        debug!(image = %image, registry = %image_ref.registry, repository = %image_ref.repository, "checking image freshness");

        match &image_ref.reference.clone() {
            ImageReference::Digest(d) => FreshnessResult::Skipped(format!(
                "digest-pinned image ({d}); skipping update check"
            )),
            ImageReference::Tag(tag) => {
                let tag = tag.clone();
                if let Some(current_version) = parse_semver_tag(&tag) {
                    self.check_semver_freshness(
                        &image_ref,
                        &tag,
                        &current_version,
                        allow_prerelease,
                    )
                    .await
                } else {
                    match non_semver_strategy {
                        NonSemverStrategy::Skip => {
                            FreshnessResult::Skipped("non-semver-strategy=skip".to_string())
                        }
                        NonSemverStrategy::Digest => {
                            self.check_digest_freshness(&image_ref, &tag, local_digest)
                                .await
                        }
                    }
                }
            }
        }
    }

    async fn check_digest_freshness(
        &self,
        image_ref: &ImageRef,
        tag: &str,
        local_digest: Option<&str>,
    ) -> FreshnessResult {
        let remote_digest = match self.fetch_manifest_digest(image_ref, tag).await {
            Ok(d) => d,
            Err(e) => {
                let should_warn = match self.head_warn_strategy {
                    HeadWarnStrategy::Always => true,
                    HeadWarnStrategy::Never => false,
                    HeadWarnStrategy::Auto => is_well_known_registry(&image_ref.registry),
                };
                if should_warn {
                    warn!(
                        registry = %image_ref.registry,
                        repository = %image_ref.repository,
                        tag,
                        error = %e,
                        "manifest digest fetch failed"
                    );
                } else {
                    debug!(
                        registry = %image_ref.registry,
                        repository = %image_ref.repository,
                        tag,
                        error = %e,
                        "manifest digest fetch failed (suppressed by head-warn-strategy)"
                    );
                }
                return FreshnessResult::Error(format!("manifest fetch failed: {e}"));
            }
        };

        let new_image = format_image_ref(image_ref, tag);
        let Some(local) = local_digest else {
            // No local digest — treat as stale so Phase 5 can pull and record it.
            return FreshnessResult::Stale(StaleInfo {
                current_digest: String::new(),
                new_image,
                new_digest: remote_digest,
            });
        };

        if normalize_digest(local) == normalize_digest(&remote_digest) {
            FreshnessResult::UpToDate
        } else {
            FreshnessResult::Stale(StaleInfo {
                current_digest: local.to_string(),
                new_image,
                new_digest: remote_digest,
            })
        }
    }

    async fn check_semver_freshness(
        &self,
        image_ref: &ImageRef,
        current_tag: &str,
        current_version: &Version,
        allow_prerelease: bool,
    ) -> FreshnessResult {
        let tags = match self.list_tags(image_ref).await {
            Ok(t) => t,
            Err(e) => return FreshnessResult::Error(format!("tag list failed: {e}")),
        };

        match find_best_semver_update(&tags, current_version, allow_prerelease) {
            None => FreshnessResult::UpToDate,
            Some((new_tag, _)) => FreshnessResult::Stale(StaleInfo {
                current_digest: current_tag.to_string(),
                new_image: format_image_ref(image_ref, new_tag),
                // Actual digest resolved during pull in Phase 5.
                new_digest: String::new(),
            }),
        }
    }

    async fn fetch_manifest_digest(
        &self,
        image_ref: &ImageRef,
        tag: &str,
    ) -> Result<String, RegistryError> {
        let url = format!(
            "{}://{}/v2/{}/manifests/{}",
            scheme_for_registry(&image_ref.registry),
            image_ref.registry,
            image_ref.repository,
            tag
        );
        debug!(registry = %image_ref.registry, repository = %image_ref.repository, tag = %tag, url = %url, "fetching manifest digest");
        let resp = self
            .do_request_with_auth(
                reqwest::Method::HEAD,
                &url,
                &image_ref.repository,
                &image_ref.registry,
            )
            .await?;
        extract_digest(resp).await
    }

    async fn list_tags(&self, image_ref: &ImageRef) -> Result<Vec<String>, RegistryError> {
        #[derive(Deserialize)]
        struct TagsResponse {
            tags: Option<Vec<String>>,
        }

        let url = format!(
            "{}://{}/v2/{}/tags/list",
            scheme_for_registry(&image_ref.registry),
            image_ref.registry,
            image_ref.repository
        );
        debug!(url = %url, "fetching tag list");
        let resp = self
            .do_request_with_auth(
                reqwest::Method::GET,
                &url,
                &image_ref.repository,
                &image_ref.registry,
            )
            .await?;

        if !resp.status().is_success() {
            return Err(RegistryError::UnexpectedResponse(format!(
                "tag list returned HTTP {}",
                resp.status()
            )));
        }

        let body: TagsResponse = resp.json().await?;
        Ok(body.tags.unwrap_or_default())
    }

    /// Single attempt: sends the request and, if the server responds with 401,
    /// fetches a bearer token and retries once with it.
    async fn send_with_auth(
        &self,
        method: reqwest::Method,
        url: &str,
        repository: &str,
        registry: &str,
    ) -> Result<reqwest::Response, RegistryError> {
        let accept = manifest_accept_header();
        let resp = self
            .client
            .request(method.clone(), url)
            .header(header::ACCEPT, &accept)
            .header(header::USER_AGENT, &self.user_agent)
            .send()
            .await?;

        if resp.status() == 401 {
            let www_auth = resp
                .headers()
                .get("www-authenticate")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();

            let token = self
                .fetch_bearer_token(&www_auth, repository, registry)
                .await?;
            let resp = self
                .client
                .request(method, url)
                .header(header::ACCEPT, &accept)
                .header(header::USER_AGENT, &self.user_agent)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .send()
                .await?;
            Ok(resp)
        } else {
            Ok(resp)
        }
    }

    /// Send a registry request, retrying up to three times on Docker Hub 429
    /// responses whose `Retry-After` value is ≤ 10 seconds.
    async fn do_request_with_auth(
        &self,
        method: reqwest::Method,
        url: &str,
        repository: &str,
        registry: &str,
    ) -> Result<reqwest::Response, RegistryError> {
        let mut attempts = 0u32;
        loop {
            let resp = self
                .send_with_auth(method.clone(), url, repository, registry)
                .await?;

            if resp.status().as_u16() != 429 || registry != "registry-1.docker.io" {
                log_rate_limit_remaining(&resp, registry);
                return Ok(resp);
            }

            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(parse_retry_after_secs);

            match rate_limit_retry_decision(retry_after, attempts) {
                Some(wait_secs) => {
                    warn!(
                        registry,
                        retry_after_secs = wait_secs,
                        attempt = attempts + 1,
                        "Docker Hub rate limited; waiting before retry"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
                    attempts += 1;
                }
                None => {
                    warn!(
                        registry,
                        retry_after = ?retry_after,
                        attempts,
                        "Docker Hub rate limited; giving up"
                    );
                    return Err(RegistryError::RateLimited { retry_after });
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) async fn test_do_request(
        &self,
        method: reqwest::Method,
        url: &str,
        repository: &str,
        registry: &str,
    ) -> Result<reqwest::Response, RegistryError> {
        self.do_request_with_auth(method, url, repository, registry)
            .await
    }

    async fn fetch_bearer_token(
        &self,
        www_auth: &str,
        repository: &str,
        registry: &str,
    ) -> Result<String, RegistryError> {
        // docker.io requires a POST to the Hub API when credentials are present.
        // Without credentials, fall through to the standard anonymous GET flow —
        // auth.docker.io/token issues anonymous tokens for public images.
        let creds = self.credentials_for(registry);
        if registry == "registry-1.docker.io" && creds.is_some() {
            return self.fetch_docker_hub_token(registry).await;
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            token: Option<String>,
            access_token: Option<String>,
        }

        let (realm, service, scope) = parse_www_authenticate(www_auth, repository);

        if realm.is_empty() {
            return Err(RegistryError::AuthFailed {
                registry: registry.to_string(),
                reason: "WWW-Authenticate header missing or unparseable".to_string(),
            });
        }

        let mut req = self
            .client
            .get(&realm)
            .header(header::USER_AGENT, &self.user_agent);
        if !service.is_empty() {
            req = req.query(&[("service", &service)]);
        }
        if !scope.is_empty() {
            req = req.query(&[("scope", &scope)]);
        }
        if let Some((ref username, ref password)) = creds {
            req = req.basic_auth(username, Some(password));
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(RegistryError::AuthFailed {
                registry: registry.to_string(),
                reason: format!("token endpoint returned HTTP {}", resp.status()),
            });
        }

        let body: TokenResponse = resp.json().await.map_err(|e| RegistryError::AuthFailed {
            registry: registry.to_string(),
            reason: format!("failed to parse token response: {e}"),
        })?;

        body.token
            .or(body.access_token)
            .ok_or_else(|| RegistryError::AuthFailed {
                registry: registry.to_string(),
                reason: "token response missing 'token' field".to_string(),
            })
    }

    async fn fetch_docker_hub_token(&self, registry: &str) -> Result<String, RegistryError> {
        let Some((ref username, ref password)) = self.credentials_for(registry) else {
            return Err(RegistryError::AuthFailed {
                registry: registry.to_string(),
                reason: "no credentials configured for docker.io".to_string(),
            });
        };

        #[derive(Deserialize)]
        struct HubTokenResponse {
            access_token: String,
        }

        let resp = self
            .client
            .post("https://hub.docker.com/v2/auth/token")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::USER_AGENT, &self.user_agent)
            .json(&serde_json::json!({
                "identifier": username,
                "secret": password,
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(RegistryError::AuthFailed {
                registry: registry.to_string(),
                reason: format!("docker.io token endpoint returned HTTP {}", resp.status()),
            });
        }

        let body: HubTokenResponse = resp.json().await.map_err(|e| RegistryError::AuthFailed {
            registry: registry.to_string(),
            reason: format!("failed to parse docker.io token response: {e}"),
        })?;

        Ok(body.access_token)
    }

    #[cfg(test)]
    pub(crate) async fn test_fetch_docker_hub_token_no_creds(
        &self,
    ) -> Result<String, RegistryError> {
        self.fetch_docker_hub_token("registry-1.docker.io").await
    }
}

// ── Pure helpers ──────────────────────────────────────────────────────────────

/// Parse a `WWW-Authenticate: Bearer ...` header into `(realm, service, scope)`.
fn parse_www_authenticate(header_value: &str, repository: &str) -> (String, String, String) {
    let rest = match header_value.trim().strip_prefix("Bearer ") {
        Some(r) => r,
        None => return (String::new(), String::new(), String::new()),
    };

    let mut realm = String::new();
    let mut service = String::new();
    let mut scope = String::new();

    for pair in split_auth_pairs(rest) {
        if let Some((key, value)) = pair.split_once('=') {
            let value = value.trim_matches('"');
            match key.trim() {
                "realm" => realm = value.to_string(),
                "service" => service = value.to_string(),
                "scope" => scope = value.to_string(),
                _ => {}
            }
        }
    }

    if scope.is_empty() {
        scope = format!("repository:{repository}:pull");
    }

    (realm, service, scope)
}

/// Split `key="value",key="value"` pairs, respecting quoted commas.
fn split_auth_pairs(s: &str) -> Vec<&str> {
    let mut pairs = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                pairs.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    pairs.push(s[start..].trim());
    pairs
}

fn manifest_accept_header() -> String {
    [
        "application/vnd.docker.distribution.manifest.v2+json",
        "application/vnd.docker.distribution.manifest.list.v2+json",
        "application/vnd.oci.image.manifest.v1+json",
        "application/vnd.oci.image.index.v1+json",
    ]
    .join(", ")
}

/// Parse a `Retry-After` header value into seconds.
/// Docker Hub always sends an integer; other formats return `None`.
pub(crate) fn parse_retry_after_secs(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok()
}

/// Decide whether to wait and retry a rate-limited request.
///
/// Returns `Some(wait_secs)` when the caller should sleep that long then retry.
/// Returns `None` when the caller should give up immediately.
pub(crate) fn rate_limit_retry_decision(retry_after: Option<u64>, attempts: u32) -> Option<u64> {
    const MAX_RETRIES: u32 = 3;
    const MAX_RETRY_AFTER_SECS: u64 = 10;
    if attempts >= MAX_RETRIES {
        return None;
    }
    match retry_after {
        Some(secs) if secs <= MAX_RETRY_AFTER_SECS => Some(secs),
        _ => None,
    }
}

/// Emit a warning when Docker Hub's rate-limit headroom drops to 5 or fewer requests.
fn log_rate_limit_remaining(resp: &reqwest::Response, registry: &str) {
    if registry != "registry-1.docker.io" {
        return;
    }
    if let Some(remaining) = resp
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        && remaining <= 5
    {
        warn!(
            registry,
            remaining, "Docker Hub rate limit nearly exhausted"
        );
    }
}

async fn extract_digest(resp: reqwest::Response) -> Result<String, RegistryError> {
    if resp.status() == 404 {
        return Err(RegistryError::ManifestNotFound(resp.url().to_string()));
    }
    if !resp.status().is_success() {
        return Err(RegistryError::UnexpectedResponse(format!(
            "HTTP {}",
            resp.status()
        )));
    }
    resp.headers()
        .get("docker-content-digest")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            RegistryError::UnexpectedResponse("missing Docker-Content-Digest header".to_string())
        })
}

pub fn is_well_known_registry(registry: &str) -> bool {
    matches!(registry, "registry-1.docker.io" | "ghcr.io")
}

fn normalize_digest(d: &str) -> &str {
    d.trim()
}

fn format_image_ref(image_ref: &ImageRef, tag: &str) -> String {
    let registry = if image_ref.registry == "registry-1.docker.io" {
        "docker.io"
    } else {
        &image_ref.registry
    };
    format!("{registry}/{}:{tag}", image_ref.repository)
}

/// Parse an image reference string into its components.
///
/// Handles the following forms:
/// - `nginx`                                   → Docker Hub official, tag `latest`
/// - `nginx:1.25.3`                            → Docker Hub official, explicit tag
/// - `myorg/myapp:1.0.0`                      → Docker Hub namespaced
/// - `ghcr.io/myorg/myapp:latest`             → custom registry
/// - `registry.example.com:5000/app`          → registry with port
/// - `myapp:1.0@sha256:abc123`                → tag+digest pin; checks tag freshness
/// - `myapp@sha256:abc123`                    → digest-only pin (no tag); always skipped
/// - `sha256:abc123`                          → bare image ID (no tag); always skipped
pub fn parse_image_ref(image: &str) -> Result<ImageRef, RegistryError> {
    // A bare image ID ("sha256:hexhash") has no registry, repository, or tag.
    // Return as a Digest reference so check_freshness skips it cleanly instead
    // of misinterpreting "sha256" as a Docker Hub repository name.
    if !image.contains('/') && image.starts_with("sha256:") {
        return Ok(ImageRef {
            registry: String::new(),
            repository: String::new(),
            reference: ImageReference::Digest(image.to_string()),
        });
    }

    let (name_part, reference) = if let Some(at_pos) = image.find('@') {
        let before_at = &image[..at_pos];
        let digest = image[at_pos + 1..].to_string();
        // If a tag is present before the @, strip it from the name and use it
        // as the reference for freshness checking. This lets `image:tag@sha256:digest`
        // references be checked for updates rather than silently skipped.
        let last_slash_end = before_at.rfind('/').map(|p| p + 1).unwrap_or(0);
        if let Some(rel_colon) = before_at[last_slash_end..].find(':') {
            let colon_pos = last_slash_end + rel_colon;
            (
                &before_at[..colon_pos],
                ImageReference::Tag(before_at[colon_pos + 1..].to_string()),
            )
        } else {
            (before_at, ImageReference::Digest(digest))
        }
    } else {
        // Find tag: last ':' in the path component after the last '/'.
        let last_slash_end = image.rfind('/').map(|p| p + 1).unwrap_or(0);
        if let Some(rel_colon) = image[last_slash_end..].find(':') {
            let colon_pos = last_slash_end + rel_colon;
            let tag = image[colon_pos + 1..].to_string();
            (&image[..colon_pos], ImageReference::Tag(tag))
        } else {
            (image, ImageReference::Tag("latest".to_string()))
        }
    };

    let (registry, repository) = split_registry_and_repo(name_part);
    Ok(ImageRef {
        registry,
        repository,
        reference,
    })
}

fn split_registry_and_repo(name: &str) -> (String, String) {
    if let Some(slash_pos) = name.find('/') {
        let first = &name[..slash_pos];
        // A path component is a registry hostname if it contains '.' or ':' or is "localhost".
        if first.contains('.') || first.contains(':') || first == "localhost" {
            let registry = if first == "docker.io" {
                "registry-1.docker.io".to_string()
            } else {
                first.to_string()
            };
            (registry, name[slash_pos + 1..].to_string())
        } else {
            ("registry-1.docker.io".to_string(), name.to_string())
        }
    } else {
        (
            "registry-1.docker.io".to_string(),
            format!("library/{name}"),
        )
    }
}

/// Parse a tag as SemVer (with optional leading `v`). Returns `None` for non-SemVer tags.
pub fn parse_semver_tag(tag: &str) -> Option<Version> {
    let s = tag.strip_prefix('v').unwrap_or(tag);
    Version::parse(s).ok()
}

/// Find the best SemVer tag to update to: highest version strictly greater than `current`.
/// Pre-release versions are excluded unless `allow_prerelease` is true.
pub fn find_best_semver_update<'a>(
    tags: &'a [String],
    current: &Version,
    allow_prerelease: bool,
) -> Option<(&'a str, Version)> {
    tags.iter()
        .filter_map(|t| parse_semver_tag(t).map(|v| (t.as_str(), v)))
        .filter(|(_, v)| v > current)
        .filter(|(_, v)| allow_prerelease || v.pre.is_empty())
        .max_by(|(_, a), (_, b)| a.cmp(b))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── scheme_for_registry ───────────────────────────────────────────────────

    #[test]
    fn scheme_for_localhost_is_http() {
        assert_eq!(scheme_for_registry("localhost"), "http");
        assert_eq!(scheme_for_registry("localhost:5000"), "http");
        assert_eq!(scheme_for_registry("127.0.0.1"), "http");
        assert_eq!(scheme_for_registry("127.0.0.1:5001"), "http");
        assert_eq!(scheme_for_registry("172.17.0.1"), "http");
        assert_eq!(scheme_for_registry("172.17.0.1:55000"), "http");
        assert_eq!(scheme_for_registry("192.168.1.100:5000"), "http");
    }

    #[test]
    fn scheme_for_remote_is_https() {
        assert_eq!(scheme_for_registry("registry-1.docker.io"), "https");
        assert_eq!(scheme_for_registry("ghcr.io"), "https");
        assert_eq!(scheme_for_registry("my-registry.example.com:443"), "https");
    }

    // ── parse_image_ref ───────────────────────────────────────────────────────

    #[test]
    fn parse_official_image_no_tag() {
        let r = parse_image_ref("nginx").unwrap();
        assert_eq!(r.registry, "registry-1.docker.io");
        assert_eq!(r.repository, "library/nginx");
        assert_eq!(r.reference, ImageReference::Tag("latest".to_string()));
    }

    #[test]
    fn parse_official_image_with_tag() {
        let r = parse_image_ref("nginx:1.25.3").unwrap();
        assert_eq!(r.registry, "registry-1.docker.io");
        assert_eq!(r.repository, "library/nginx");
        assert_eq!(r.reference, ImageReference::Tag("1.25.3".to_string()));
    }

    #[test]
    fn parse_namespaced_image() {
        let r = parse_image_ref("myorg/myapp:1.0.0").unwrap();
        assert_eq!(r.registry, "registry-1.docker.io");
        assert_eq!(r.repository, "myorg/myapp");
        assert_eq!(r.reference, ImageReference::Tag("1.0.0".to_string()));
    }

    #[test]
    fn parse_custom_registry() {
        let r = parse_image_ref("ghcr.io/myorg/myapp:latest").unwrap();
        assert_eq!(r.registry, "ghcr.io");
        assert_eq!(r.repository, "myorg/myapp");
        assert_eq!(r.reference, ImageReference::Tag("latest".to_string()));
    }

    #[test]
    fn parse_registry_with_port() {
        let r = parse_image_ref("registry.example.com:5000/myapp:v2").unwrap();
        assert_eq!(r.registry, "registry.example.com:5000");
        assert_eq!(r.repository, "myapp");
        assert_eq!(r.reference, ImageReference::Tag("v2".to_string()));
    }

    #[test]
    fn parse_digest_pinned() {
        let r = parse_image_ref("nginx@sha256:abc123def456").unwrap();
        assert_eq!(r.registry, "registry-1.docker.io");
        assert_eq!(r.repository, "library/nginx");
        assert_eq!(
            r.reference,
            ImageReference::Digest("sha256:abc123def456".to_string())
        );
    }

    #[test]
    fn parse_tag_and_digest_uses_tag() {
        let r = parse_image_ref("nginx:latest@sha256:abc123").unwrap();
        assert_eq!(r.registry, "registry-1.docker.io");
        assert_eq!(r.repository, "library/nginx");
        assert_eq!(r.reference, ImageReference::Tag("latest".to_string()));
    }

    #[test]
    fn parse_tag_and_digest_custom_registry() {
        let r = parse_image_ref("lscr.io/linuxserver/sonarr:latest@sha256:abc123def456").unwrap();
        assert_eq!(r.registry, "lscr.io");
        assert_eq!(r.repository, "linuxserver/sonarr");
        assert_eq!(r.reference, ImageReference::Tag("latest".to_string()));
    }

    #[test]
    fn parse_bare_sha256_digest() {
        let r = parse_image_ref(
            "sha256:ddf896fd2d36a9798a6dd9b9c404c34a99f54aff1881f80292aba7c4247a914c",
        )
        .unwrap();
        assert_eq!(r.registry, "");
        assert_eq!(r.repository, "");
        assert_eq!(
            r.reference,
            ImageReference::Digest(
                "sha256:ddf896fd2d36a9798a6dd9b9c404c34a99f54aff1881f80292aba7c4247a914c"
                    .to_string()
            )
        );
    }

    #[test]
    fn parse_docker_io_normalised() {
        let r = parse_image_ref("docker.io/library/nginx:latest").unwrap();
        assert_eq!(r.registry, "registry-1.docker.io");
        assert_eq!(r.repository, "library/nginx");
    }

    #[test]
    fn parse_no_tag_defaults_to_latest() {
        let r = parse_image_ref("myorg/myapp").unwrap();
        assert_eq!(r.reference, ImageReference::Tag("latest".to_string()));
    }

    #[test]
    fn parse_localhost_registry() {
        let r = parse_image_ref("localhost/myapp:dev").unwrap();
        assert_eq!(r.registry, "localhost");
        assert_eq!(r.repository, "myapp");
        assert_eq!(r.reference, ImageReference::Tag("dev".to_string()));
    }

    // ── parse_semver_tag ──────────────────────────────────────────────────────

    #[test]
    fn semver_tag_plain() {
        assert_eq!(parse_semver_tag("1.2.3"), Some(Version::new(1, 2, 3)));
    }

    #[test]
    fn semver_tag_v_prefix() {
        assert_eq!(parse_semver_tag("v1.2.3"), Some(Version::new(1, 2, 3)));
    }

    #[test]
    fn semver_tag_prerelease() {
        let v = parse_semver_tag("1.2.3-beta.1").unwrap();
        assert_eq!(v.major, 1);
        assert!(!v.pre.is_empty());
    }

    #[test]
    fn semver_tag_non_semver_latest() {
        assert_eq!(parse_semver_tag("latest"), None);
    }

    #[test]
    fn semver_tag_two_part_version() {
        // "1.25" is not strict semver (missing patch)
        assert_eq!(parse_semver_tag("1.25"), None);
    }

    #[test]
    fn semver_tag_non_semver_date() {
        assert_eq!(parse_semver_tag("20240101"), None);
    }

    // ── find_best_semver_update ───────────────────────────────────────────────

    fn tags(s: &[&str]) -> Vec<String> {
        s.iter().map(|t| t.to_string()).collect()
    }

    #[test]
    fn semver_update_empty_tags() {
        let current = Version::new(1, 0, 0);
        assert!(find_best_semver_update(&[], &current, false).is_none());
    }

    #[test]
    fn semver_update_no_newer() {
        let current = Version::new(2, 0, 0);
        let t = tags(&["1.0.0", "1.9.9"]);
        assert!(find_best_semver_update(&t, &current, false).is_none());
    }

    #[test]
    fn semver_update_one_newer() {
        let current = Version::new(1, 0, 0);
        let t = tags(&["1.0.0", "1.1.0"]);
        let (tag, _) = find_best_semver_update(&t, &current, false).unwrap();
        assert_eq!(tag, "1.1.0");
    }

    #[test]
    fn semver_update_picks_highest() {
        let current = Version::new(1, 0, 0);
        let t = tags(&["1.1.0", "1.2.0", "1.0.1", "latest"]);
        let (tag, _) = find_best_semver_update(&t, &current, false).unwrap();
        assert_eq!(tag, "1.2.0");
    }

    #[test]
    fn semver_update_excludes_prerelease_by_default() {
        let current = Version::new(1, 0, 0);
        let t = tags(&["1.1.0-beta.1", "1.0.1"]);
        let (tag, _) = find_best_semver_update(&t, &current, false).unwrap();
        assert_eq!(tag, "1.0.1");
    }

    #[test]
    fn semver_update_prerelease_only_no_result_without_flag() {
        let current = Version::new(1, 0, 0);
        let t = tags(&["1.1.0-beta.1"]);
        assert!(find_best_semver_update(&t, &current, false).is_none());
    }

    #[test]
    fn semver_update_allows_prerelease_with_flag() {
        let current = Version::new(1, 0, 0);
        let t = tags(&["1.1.0-beta.1"]);
        let (tag, _) = find_best_semver_update(&t, &current, true).unwrap();
        assert_eq!(tag, "1.1.0-beta.1");
    }

    #[test]
    fn semver_update_v_prefix_tags_handled() {
        let current = Version::new(1, 0, 0);
        let t = tags(&["v1.1.0", "v1.2.0"]);
        let (tag, _) = find_best_semver_update(&t, &current, false).unwrap();
        assert_eq!(tag, "v1.2.0");
    }

    // ── parse_non_semver_strategy ─────────────────────────────────────────────

    #[test]
    fn non_semver_strategy_skip() {
        assert_eq!(parse_non_semver_strategy("skip"), NonSemverStrategy::Skip);
    }

    #[test]
    fn non_semver_strategy_digest() {
        assert_eq!(
            parse_non_semver_strategy("digest"),
            NonSemverStrategy::Digest
        );
    }

    #[test]
    fn non_semver_strategy_default_fallback() {
        assert_eq!(
            parse_non_semver_strategy("unknown"),
            NonSemverStrategy::Digest
        );
        assert_eq!(parse_non_semver_strategy(""), NonSemverStrategy::Digest);
    }

    #[test]
    fn non_semver_strategy_case_insensitive() {
        assert_eq!(parse_non_semver_strategy("SKIP"), NonSemverStrategy::Skip);
    }

    // ── is_well_known_registry ────────────────────────────────────────────────

    #[test]
    fn well_known_docker_hub() {
        assert!(is_well_known_registry("registry-1.docker.io"));
    }

    #[test]
    fn well_known_ghcr() {
        assert!(is_well_known_registry("ghcr.io"));
    }

    #[test]
    fn not_well_known_custom() {
        assert!(!is_well_known_registry("registry.example.com"));
        assert!(!is_well_known_registry("docker.io"));
    }

    // ── parse_www_authenticate ────────────────────────────────────────────────

    #[test]
    fn www_auth_full_header() {
        let header = r#"Bearer realm="https://auth.docker.io/token",service="registry.docker.io",scope="repository:library/nginx:pull""#;
        let (realm, service, scope) = parse_www_authenticate(header, "library/nginx");
        assert_eq!(realm, "https://auth.docker.io/token");
        assert_eq!(service, "registry.docker.io");
        assert_eq!(scope, "repository:library/nginx:pull");
    }

    #[test]
    fn www_auth_missing_scope_uses_default() {
        let header = r#"Bearer realm="https://auth.docker.io/token",service="registry.docker.io""#;
        let (realm, service, scope) = parse_www_authenticate(header, "library/nginx");
        assert_eq!(realm, "https://auth.docker.io/token");
        assert_eq!(service, "registry.docker.io");
        assert_eq!(scope, "repository:library/nginx:pull");
    }

    #[test]
    fn www_auth_not_bearer_returns_empty() {
        let (realm, _, _) = parse_www_authenticate("Basic realm=\"registry\"", "repo");
        assert!(realm.is_empty());
    }

    #[test]
    fn www_auth_unknown_key_is_ignored() {
        let header = r#"Bearer realm="https://auth.docker.io/token",service="registry.docker.io",unknown_key="value""#;
        let (realm, service, _) = parse_www_authenticate(header, "myrepo");
        assert_eq!(realm, "https://auth.docker.io/token");
        assert_eq!(service, "registry.docker.io");
    }

    // ── split_registry_and_repo ───────────────────────────────────────────────

    #[test]
    fn split_registry_and_repo_official_image() {
        let (reg, repo) = split_registry_and_repo("nginx");
        assert_eq!(reg, "registry-1.docker.io");
        assert_eq!(repo, "library/nginx");
    }

    #[test]
    fn split_registry_and_repo_namespaced() {
        let (reg, repo) = split_registry_and_repo("myorg/myapp");
        assert_eq!(reg, "registry-1.docker.io");
        assert_eq!(repo, "myorg/myapp");
    }

    #[test]
    fn split_registry_and_repo_localhost() {
        let (reg, repo) = split_registry_and_repo("localhost/myapp");
        assert_eq!(reg, "localhost");
        assert_eq!(repo, "myapp");
    }

    // ── split_auth_pairs ──────────────────────────────────────────────────────

    #[test]
    fn split_auth_pairs_quoted_commas_not_split() {
        let pairs = split_auth_pairs(r#"realm="https://auth.io",scope="repository:foo,bar:pull""#);
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], r#"realm="https://auth.io""#);
        assert_eq!(pairs[1], r#"scope="repository:foo,bar:pull""#);
    }

    // ── format_image_ref ─────────────────────────────────────────────────────

    #[test]
    fn format_image_ref_docker_hub_normalizes_to_docker_io() {
        let image_ref = ImageRef {
            registry: "registry-1.docker.io".to_string(),
            repository: "myorg/myapp".to_string(),
            reference: ImageReference::Tag("1.0.0".to_string()),
        };
        assert_eq!(
            format_image_ref(&image_ref, "1.0.0"),
            "docker.io/myorg/myapp:1.0.0"
        );
    }

    #[test]
    fn format_image_ref_custom_registry() {
        let image_ref = ImageRef {
            registry: "ghcr.io".to_string(),
            repository: "myorg/myapp".to_string(),
            reference: ImageReference::Tag("latest".to_string()),
        };
        assert_eq!(
            format_image_ref(&image_ref, "latest"),
            "ghcr.io/myorg/myapp:latest"
        );
    }

    // ── normalize_digest ──────────────────────────────────────────────────────

    #[test]
    fn normalize_digest_trims_whitespace() {
        assert_eq!(normalize_digest("  sha256:abc  "), "sha256:abc");
    }

    #[test]
    fn normalize_digest_no_whitespace_unchanged() {
        assert_eq!(normalize_digest("sha256:abc"), "sha256:abc");
    }

    // ── manifest_accept_header ────────────────────────────────────────────────

    #[test]
    fn manifest_accept_header_contains_oci_and_docker_types() {
        let h = manifest_accept_header();
        assert!(h.contains("application/vnd.docker.distribution.manifest.v2+json"));
        assert!(h.contains("application/vnd.oci.image.manifest.v1+json"));
        assert!(h.contains("application/vnd.oci.image.index.v1+json"));
    }

    // ── docker hub token auth ─────────────────────────────────────────────────

    // ── normalize_registry_host ───────────────────────────────────────────────

    #[test]
    fn normalize_docker_io_to_registry_docker_io() {
        assert_eq!(normalize_registry_host("docker.io"), "registry-1.docker.io");
    }

    #[test]
    fn normalize_registry_docker_io_unchanged() {
        assert_eq!(
            normalize_registry_host("registry-1.docker.io"),
            "registry-1.docker.io"
        );
    }

    #[test]
    fn normalize_other_host_unchanged() {
        assert_eq!(normalize_registry_host("ghcr.io"), "ghcr.io");
        assert_eq!(normalize_registry_host("quay.io"), "quay.io");
    }

    // ── credentials_for ───────────────────────────────────────────────────────

    fn client_with(
        global: Option<(&str, &str)>,
        per: Vec<(&str, Option<(&str, &str)>)>,
    ) -> RegistryClient {
        let global = global.map(|(u, p)| (u.to_string(), p.to_string()));
        let per_entries = per
            .into_iter()
            .map(|(host, c)| {
                (
                    host.to_string(),
                    c.map(|(u, p)| (u.to_string(), p.to_string())),
                )
            })
            .collect();
        RegistryClient::new(HeadWarnStrategy::Auto, "test", global, per_entries).unwrap()
    }

    #[test]
    fn credentials_for_uses_per_registry_when_present() {
        let c = client_with(
            Some(("global_user", "global_pass")),
            vec![("ghcr.io", Some(("ghcr_user", "ghcr_pass")))],
        );
        assert_eq!(
            c.credentials_for("ghcr.io"),
            Some(("ghcr_user".into(), "ghcr_pass".into()))
        );
    }

    #[test]
    fn credentials_for_falls_back_to_global_when_no_per_registry_entry() {
        let c = client_with(Some(("global_user", "global_pass")), vec![]);
        assert_eq!(
            c.credentials_for("ghcr.io"),
            Some(("global_user".into(), "global_pass".into()))
        );
    }

    #[test]
    fn credentials_for_explicit_anonymous_overrides_global() {
        let c = client_with(
            Some(("global_user", "global_pass")),
            vec![("quay.io", None)],
        );
        assert_eq!(c.credentials_for("quay.io"), None);
    }

    #[test]
    fn credentials_for_anonymous_when_no_entry_and_no_global() {
        let c = client_with(None, vec![]);
        assert_eq!(c.credentials_for("ghcr.io"), None);
    }

    #[test]
    fn credentials_for_normalizes_docker_io_on_insert() {
        let c = client_with(None, vec![("docker.io", Some(("u", "p")))]);
        assert_eq!(
            c.credentials_for("registry-1.docker.io"),
            Some(("u".into(), "p".into()))
        );
    }

    // ── credentials_for_image ─────────────────────────────────────────────────

    #[test]
    fn credentials_for_image_resolves_ghcr_image() {
        let c = client_with(None, vec![("ghcr.io", Some(("ghcr_user", "ghcr_token")))]);
        assert_eq!(
            c.credentials_for_image("ghcr.io/org/repo:latest"),
            Some(("ghcr_user".into(), "ghcr_token".into()))
        );
    }

    #[test]
    fn credentials_for_image_falls_back_to_global_for_unlisted_registry() {
        let c = client_with(Some(("global_user", "global_pass")), vec![]);
        assert_eq!(
            c.credentials_for_image("quay.io/org/image:latest"),
            Some(("global_user".into(), "global_pass".into()))
        );
    }

    // ── docker hub token auth ─────────────────────────────────────────────────

    #[tokio::test]
    async fn docker_hub_token_no_credentials_returns_auth_failed() {
        let client = RegistryClient::new(HeadWarnStrategy::Auto, "test", None, vec![]).unwrap();
        let err = client
            .test_fetch_docker_hub_token_no_creds()
            .await
            .unwrap_err();
        assert!(
            matches!(err, RegistryError::AuthFailed { .. }),
            "expected AuthFailed, got {err:?}"
        );
    }

    // ── parse_retry_after_secs ────────────────────────────────────────────────

    #[test]
    fn parse_retry_after_valid_integer() {
        assert_eq!(parse_retry_after_secs("5"), Some(5));
        assert_eq!(parse_retry_after_secs("10"), Some(10));
        assert_eq!(parse_retry_after_secs("120"), Some(120));
    }

    #[test]
    fn parse_retry_after_trims_whitespace() {
        assert_eq!(parse_retry_after_secs(" 7 "), Some(7));
    }

    #[test]
    fn parse_retry_after_zero() {
        assert_eq!(parse_retry_after_secs("0"), Some(0));
    }

    #[test]
    fn parse_retry_after_empty_is_none() {
        assert_eq!(parse_retry_after_secs(""), None);
    }

    #[test]
    fn parse_retry_after_non_numeric_is_none() {
        assert_eq!(parse_retry_after_secs("abc"), None);
        assert_eq!(parse_retry_after_secs("1.5"), None);
    }

    #[test]
    fn parse_retry_after_overflow_is_none() {
        assert_eq!(parse_retry_after_secs("99999999999999999999"), None);
    }

    // ── rate_limit_retry_decision ─────────────────────────────────────────────

    #[test]
    fn rate_limit_retry_within_limit_returns_wait() {
        assert_eq!(rate_limit_retry_decision(Some(5), 0), Some(5));
    }

    #[test]
    fn rate_limit_retry_at_exactly_10s_retries() {
        assert_eq!(rate_limit_retry_decision(Some(10), 0), Some(10));
    }

    #[test]
    fn rate_limit_retry_11s_exceeds_limit() {
        assert_eq!(rate_limit_retry_decision(Some(11), 0), None);
    }

    #[test]
    fn rate_limit_retry_no_header_gives_up() {
        assert_eq!(rate_limit_retry_decision(None, 0), None);
    }

    #[test]
    fn rate_limit_retry_last_allowed_attempt() {
        // attempts == 2 is the third attempt (0-indexed), which is still allowed
        assert_eq!(rate_limit_retry_decision(Some(5), 2), Some(5));
    }

    #[test]
    fn rate_limit_retry_at_max_retries_gives_up() {
        assert_eq!(rate_limit_retry_decision(Some(5), 3), None);
    }

    #[test]
    fn rate_limit_retry_past_max_retries_gives_up() {
        assert_eq!(rate_limit_retry_decision(Some(5), 10), None);
    }

    // ── 429 retry loop (mock server) ──────────────────────────────────────────

    fn make_429_response(retry_after: Option<u64>) -> axum::response::Response {
        let mut builder =
            axum::response::Response::builder().status(axum::http::StatusCode::TOO_MANY_REQUESTS);
        if let Some(secs) = retry_after {
            builder = builder.header("retry-after", secs.to_string());
        }
        builder.body(axum::body::Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn rate_limit_retries_then_succeeds() {
        use axum::{Router, routing::head};
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let call_count = Arc::new(Mutex::new(0u32));
        let cc = Arc::clone(&call_count);

        let app = Router::new().route(
            "/v2/library/nginx/manifests/latest",
            head(move || {
                let cc = Arc::clone(&cc);
                async move {
                    let mut n = cc.lock().await;
                    *n += 1;
                    if *n == 1 {
                        make_429_response(Some(0))
                    } else {
                        axum::response::Response::builder()
                            .status(200)
                            .header("docker-content-digest", "sha256:abc123")
                            .body(axum::body::Body::empty())
                            .unwrap()
                    }
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let client = RegistryClient::new(HeadWarnStrategy::Auto, "test", None, vec![]).unwrap();
        let url = format!("http://127.0.0.1:{port}/v2/library/nginx/manifests/latest");
        let resp = client
            .test_do_request(
                reqwest::Method::HEAD,
                &url,
                "library/nginx",
                "registry-1.docker.io",
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        assert_eq!(*call_count.lock().await, 2);
    }

    #[tokio::test]
    async fn rate_limit_gives_up_when_retry_after_too_large() {
        use axum::{Router, routing::head};

        let app = Router::new().route(
            "/v2/library/nginx/manifests/latest",
            head(|| async { make_429_response(Some(11)) }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let client = RegistryClient::new(HeadWarnStrategy::Auto, "test", None, vec![]).unwrap();
        let url = format!("http://127.0.0.1:{port}/v2/library/nginx/manifests/latest");
        let err = client
            .test_do_request(
                reqwest::Method::HEAD,
                &url,
                "library/nginx",
                "registry-1.docker.io",
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                RegistryError::RateLimited {
                    retry_after: Some(11)
                }
            ),
            "unexpected error: {err:?}"
        );
    }

    #[tokio::test]
    async fn rate_limit_gives_up_when_no_retry_after_header() {
        use axum::{Router, routing::head};

        let app = Router::new().route(
            "/v2/library/nginx/manifests/latest",
            head(|| async { make_429_response(None) }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let client = RegistryClient::new(HeadWarnStrategy::Auto, "test", None, vec![]).unwrap();
        let url = format!("http://127.0.0.1:{port}/v2/library/nginx/manifests/latest");
        let err = client
            .test_do_request(
                reqwest::Method::HEAD,
                &url,
                "library/nginx",
                "registry-1.docker.io",
            )
            .await
            .unwrap_err();

        assert!(
            matches!(err, RegistryError::RateLimited { retry_after: None }),
            "unexpected error: {err:?}"
        );
    }

    #[tokio::test]
    async fn rate_limit_exhausts_retries_after_three_attempts() {
        use axum::{Router, routing::head};
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let call_count = Arc::new(Mutex::new(0u32));
        let cc = Arc::clone(&call_count);

        // Always returns 429 with Retry-After: 0 so the test is instant.
        let app = Router::new().route(
            "/v2/library/nginx/manifests/latest",
            head(move || {
                let cc = Arc::clone(&cc);
                async move {
                    *cc.lock().await += 1;
                    make_429_response(Some(0))
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let client = RegistryClient::new(HeadWarnStrategy::Auto, "test", None, vec![]).unwrap();
        let url = format!("http://127.0.0.1:{port}/v2/library/nginx/manifests/latest");
        let err = client
            .test_do_request(
                reqwest::Method::HEAD,
                &url,
                "library/nginx",
                "registry-1.docker.io",
            )
            .await
            .unwrap_err();

        assert!(
            matches!(err, RegistryError::RateLimited { .. }),
            "unexpected error: {err:?}"
        );
        // 1 initial + 3 retries = 4 total requests
        assert_eq!(*call_count.lock().await, 4);
    }

    // ── proptest ──────────────────────────────────────────────────────────────

    #[cfg(test)]
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// The best SemVer update is always strictly greater than current.
            #[test]
            fn prop_semver_best_is_strictly_greater(
                current_maj in 0u64..10,
                current_min in 0u64..10,
                current_pat in 0u64..10,
                tags in proptest::collection::vec(
                    "[0-9]{1,2}\\.[0-9]{1,2}\\.[0-9]{1,2}",
                    0..30,
                ),
            ) {
                let current = Version::new(current_maj, current_min, current_pat);
                if let Some((_, best)) = find_best_semver_update(&tags, &current, false) {
                    prop_assert!(best > current);
                    prop_assert!(best.pre.is_empty());
                }
            }

            /// When allow_prerelease=true, the result is still > current.
            #[test]
            fn prop_semver_prerelease_still_greater(
                current_maj in 0u64..10,
                current_min in 0u64..10,
                current_pat in 0u64..10,
                tags in proptest::collection::vec(
                    "[0-9]{1,2}\\.[0-9]{1,2}\\.[0-9]{1,2}(-[a-z]{1,5})?",
                    0..30,
                ),
            ) {
                let current = Version::new(current_maj, current_min, current_pat);
                if let Some((_, best)) = find_best_semver_update(&tags, &current, true) {
                    prop_assert!(best > current);
                }
            }

            /// parse_image_ref never panics on arbitrary input.
            #[test]
            fn prop_parse_image_ref_no_panic(image in ".*") {
                let _ = parse_image_ref(&image);
            }

            /// For known-valid image ref forms, parse_image_ref always succeeds.
            #[test]
            fn prop_valid_image_refs_parse(
                name in "[a-z][a-z0-9]{0,10}",
                tag in "[a-z0-9][a-z0-9\\.\\-]{0,10}",
            ) {
                // "name:tag" form
                let image = format!("{name}:{tag}");
                let r = parse_image_ref(&image).unwrap();
                assert_eq!(r.registry, "registry-1.docker.io");
                assert_eq!(r.repository, format!("library/{name}"));
                assert_eq!(r.reference, ImageReference::Tag(tag));
            }
        }
    }
}
