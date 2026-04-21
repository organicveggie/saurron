use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use bollard::API_DEFAULT_VERSION;
use bollard::Docker;

use crate::config::DockerConfig;

pub struct DockerClient {
    inner: Docker,
}

#[derive(Debug, PartialEq)]
enum ConnectionType {
    Socket,
    Http,
    Https,
}

fn connection_type(host: &str, tls_verify: bool) -> ConnectionType {
    if host.starts_with("unix://") || host.starts_with("npipe://") || host.starts_with('/') {
        ConnectionType::Socket
    } else if tls_verify || host.starts_with("https://") {
        ConnectionType::Https
    } else {
        ConnectionType::Http
    }
}

fn parse_api_version(version: &str) -> Result<bollard::ClientVersion> {
    let v = version.trim_start_matches('v');
    let (major_str, minor_str) = v
        .split_once('.')
        .context("API version must be in 'major.minor' format")?;
    let major_version = major_str
        .parse::<usize>()
        .context("invalid API version major component")?;
    let minor_version = minor_str
        .parse::<usize>()
        .context("invalid API version minor component")?;
    Ok(bollard::ClientVersion {
        major_version,
        minor_version,
    })
}

// ── Container state ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ContainerState {
    Created,
    Restarting,
    Running,
    Removing,
    Paused,
    Exited,
    Dead,
    Unknown(String),
}

impl ContainerState {
    pub fn from_str(s: &str) -> Self {
        match s {
            "created" => Self::Created,
            "restarting" => Self::Restarting,
            "running" => Self::Running,
            "removing" => Self::Removing,
            "paused" => Self::Paused,
            "exited" => Self::Exited,
            "dead" => Self::Dead,
            other => Self::Unknown(other.to_string()),
        }
    }
}

impl std::fmt::Display for ContainerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Created => "created",
            Self::Restarting => "restarting",
            Self::Running => "running",
            Self::Removing => "removing",
            Self::Paused => "paused",
            Self::Exited => "exited",
            Self::Dead => "dead",
            Self::Unknown(s) => s,
        };
        f.write_str(s)
    }
}

// ── ContainerInfo ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub image_id: String,
    pub state: ContainerState,
    pub labels: HashMap<String, String>,
}

impl ContainerInfo {
    pub fn saurron_labels(&self) -> SaurronLabels {
        SaurronLabels::from_labels(&self.labels)
    }
}

// ── Saurron per-container labels ──────────────────────────────────────────────

const LABEL_ENABLE: &str = "saurron.enable";
const LABEL_SCOPE: &str = "saurron.scope";
const LABEL_DEPENDS_ON: &str = "saurron.depends-on";
const LABEL_IMAGE_TAG: &str = "saurron.image-tag";
const LABEL_SEMVER_PRE_RELEASE: &str = "saurron.semver-pre-release";
const LABEL_NON_SEMVER_STRATEGY: &str = "saurron.non-semver-strategy";
const LABEL_MONITOR_ONLY: &str = "saurron.monitor-only";
const LABEL_NO_PULL: &str = "saurron.no-pull";
const LABEL_STOP_SIGNAL: &str = "saurron.stop-signal";
const LABEL_STOP_TIMEOUT: &str = "saurron.stop-timeout";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SaurronLabels {
    pub enable: Option<bool>,
    pub scope: Option<String>,
    pub depends_on: Vec<String>,
    pub image_tag: Option<String>,
    /// Include pre-release versions when selecting the latest SemVer tag.
    pub semver_pre_release: Option<bool>,
    /// Override non-semver tag strategy: `"digest"` (default) or `"skip"`.
    pub non_semver_strategy: Option<String>,
    /// Detect + notify only; do not pull or restart.
    pub monitor_only: Option<bool>,
    /// Restart from cached image without pulling.
    pub no_pull: Option<bool>,
    /// Override stop signal (e.g. `"SIGHUP"`).
    pub stop_signal: Option<String>,
    /// Override graceful stop timeout (e.g. `"30s"`).
    pub stop_timeout: Option<String>,
}

impl SaurronLabels {
    pub fn from_labels(labels: &HashMap<String, String>) -> Self {
        Self {
            enable: labels.get(LABEL_ENABLE).and_then(|v| parse_bool_label(v)),
            scope: labels.get(LABEL_SCOPE).filter(|v| !v.is_empty()).cloned(),
            depends_on: labels
                .get(LABEL_DEPENDS_ON)
                .map(|v| parse_depends_on(v))
                .unwrap_or_default(),
            image_tag: labels
                .get(LABEL_IMAGE_TAG)
                .filter(|v| !v.is_empty())
                .cloned(),
            semver_pre_release: labels
                .get(LABEL_SEMVER_PRE_RELEASE)
                .and_then(|v| parse_bool_label(v)),
            non_semver_strategy: labels
                .get(LABEL_NON_SEMVER_STRATEGY)
                .filter(|v| !v.is_empty())
                .cloned(),
            monitor_only: labels.get(LABEL_MONITOR_ONLY).and_then(|v| parse_bool_label(v)),
            no_pull: labels.get(LABEL_NO_PULL).and_then(|v| parse_bool_label(v)),
            stop_signal: labels.get(LABEL_STOP_SIGNAL).filter(|v| !v.is_empty()).cloned(),
            stop_timeout: labels.get(LABEL_STOP_TIMEOUT).filter(|v| !v.is_empty()).cloned(),
        }
    }
}

fn parse_bool_label(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_depends_on(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// ── Container selection ───────────────────────────────────────────────────────

pub struct ContainerSelector {
    label_enable: bool,
    global_takes_precedence: bool,
    disabled_names: HashSet<String>,
    allowed_names: Option<HashSet<String>>,
    include_restarting: bool,
    revive_stopped: bool,
}

impl ContainerSelector {
    pub fn new(
        label_enable: bool,
        global_takes_precedence: bool,
        disable_containers: &[String],
        containers: &[String],
        include_restarting: bool,
        revive_stopped: bool,
    ) -> Self {
        Self {
            label_enable,
            global_takes_precedence,
            disabled_names: disable_containers.iter().cloned().collect(),
            allowed_names: if containers.is_empty() {
                None
            } else {
                Some(containers.iter().cloned().collect())
            },
            include_restarting,
            revive_stopped,
        }
    }

    /// Returns the Docker container state strings to pass as list filters.
    pub fn state_filter(&self) -> Vec<&'static str> {
        let mut states = vec!["running"];
        if self.include_restarting {
            states.push("restarting");
        }
        if self.revive_stopped {
            states.push("exited");
            states.push("created");
        }
        states
    }

    /// Returns true if this container should be included in the update cycle.
    pub fn is_selected(&self, container: &ContainerInfo) -> bool {
        if let Some(ref allowed) = self.allowed_names {
            if !allowed.contains(&container.name) {
                return false;
            }
        }

        if self.disabled_names.contains(&container.name) {
            return false;
        }

        let labels = container.saurron_labels();

        if self.label_enable {
            // Opt-in: only containers explicitly enabled via label
            matches!(labels.enable, Some(true))
        } else if self.global_takes_precedence {
            // Opt-out with global precedence: per-container disable label is ignored
            true
        } else {
            // Opt-out: include unless the container opts out via label
            !matches!(labels.enable, Some(false))
        }
    }

    pub fn select(&self, containers: &[ContainerInfo]) -> Vec<ContainerInfo> {
        containers
            .iter()
            .filter(|c| self.is_selected(c))
            .cloned()
            .collect()
    }
}

// ── Local image info ──────────────────────────────────────────────────────────

/// Canonical name and manifest digest of a locally-present image.
#[derive(Debug, Default, PartialEq)]
pub struct LocalImageInfo {
    /// First `RepoTags` entry, e.g. `"postgres:15"`.
    pub name: Option<String>,
    /// Manifest digest from first `RepoDigests` entry (part after `@`),
    /// e.g. `"sha256:6eed15406dbba206cb1260528a3354d80d2522cab068cb9ad7a1ede5ac90e6f6"`.
    pub digest: Option<String>,
}

fn local_image_info_from_inspect(inspect: &bollard::models::ImageInspect) -> LocalImageInfo {
    let name = inspect
        .repo_tags
        .as_deref()
        .and_then(|tags| tags.first())
        .cloned();
    let digest = inspect
        .repo_digests
        .as_deref()
        .and_then(|digests| digests.first())
        .and_then(|rd| rd.split_once('@').map(|(_, d)| d.to_string()));
    LocalImageInfo { name, digest }
}

// ── Bollard summary → ContainerInfo ──────────────────────────────────────────

fn summary_to_info(s: bollard::models::ContainerSummary) -> Option<ContainerInfo> {
    Some(ContainerInfo {
        id: s.id?,
        name: s
            .names
            .as_deref()
            .and_then(|v| v.first())
            .map(|n| n.trim_start_matches('/').to_string())
            .unwrap_or_default(),
        image: s.image.unwrap_or_default(),
        image_id: s.image_id.unwrap_or_default(),
        state: ContainerState::from_str(s.state.as_deref().unwrap_or("unknown")),
        labels: s.labels.unwrap_or_default(),
    })
}

// ── Docker client ─────────────────────────────────────────────────────────────

impl DockerClient {
    pub fn connect(config: &DockerConfig) -> Result<Self> {
        let api_version = config
            .api_version
            .as_deref()
            .map(parse_api_version)
            .transpose()?;
        let version = api_version.as_ref().unwrap_or(API_DEFAULT_VERSION);

        let inner = match connection_type(&config.host, config.tls_verify) {
            ConnectionType::Socket => Docker::connect_with_socket(&config.host, 120, version)
                .context("failed to connect to Docker via Unix socket")?,

            ConnectionType::Http => Docker::connect_with_http(&config.host, 120, version)
                .context("failed to connect to Docker via HTTP")?,

            ConnectionType::Https => {
                let ca = config
                    .tls_ca_cert
                    .as_deref()
                    .context("--tls-ca-cert is required when --tlsverify is set")?;

                let (key_path, cert_path): (PathBuf, PathBuf) =
                    match (&config.tls_key, &config.tls_cert) {
                        (Some(k), Some(c)) => (PathBuf::from(k), PathBuf::from(c)),
                        (None, None) => {
                            // Server-only TLS: bollard resolver returns None → no client auth
                            (PathBuf::new(), PathBuf::new())
                        }
                        _ => bail!("--tls-cert and --tls-key must be provided together"),
                    };

                Docker::connect_with_ssl(
                    &config.host,
                    &key_path,
                    &cert_path,
                    Path::new(ca),
                    120,
                    version,
                )
                .context("failed to connect to Docker via TLS")?
            }
        };

        Ok(Self { inner })
    }

    pub async fn ping(&self) -> Result<()> {
        self.inner
            .ping()
            .await
            .context("Docker daemon ping failed")?;
        Ok(())
    }

    pub async fn list_containers(
        &self,
        selector: &ContainerSelector,
    ) -> Result<Vec<ContainerInfo>> {
        use bollard::container::ListContainersOptions;
        let status_filter: Vec<String> = selector
            .state_filter()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut filters = HashMap::new();
        filters.insert("status".to_string(), status_filter);
        let opts = ListContainersOptions {
            all: true,
            filters,
            ..Default::default()
        };
        let summaries = self
            .inner
            .list_containers(Some(opts))
            .await
            .context("failed to list containers")?;
        Ok(summaries.into_iter().filter_map(summary_to_info).collect())
    }

    pub fn select_containers(
        &self,
        containers: &[ContainerInfo],
        selector: &ContainerSelector,
    ) -> Vec<ContainerInfo> {
        selector.select(containers)
    }

    /// Inspects a local image (by name or sha256 ID) and returns its canonical
    /// name and manifest digest.
    ///
    /// `name` is taken from the first `RepoTags` entry (e.g. `"postgres:15"`).
    /// `digest` is taken from the first `RepoDigests` entry after the `@`
    /// separator (e.g. `"sha256:6eed15..."`).
    ///
    /// Either field may be `None` for dangling or locally-built images.
    pub async fn get_local_image_info(&self, image: &str) -> Result<LocalImageInfo> {
        let inspect = self
            .inner
            .inspect_image(image)
            .await
            .with_context(|| format!("failed to inspect image '{image}'"))?;

        Ok(local_image_info_from_inspect(&inspect))
    }

    pub async fn inspect_container(
        &self,
        id: &str,
    ) -> Result<bollard::models::ContainerInspectResponse> {
        self.inner
            .inspect_container(id, None)
            .await
            .with_context(|| format!("failed to inspect container '{id}'"))
    }

    pub async fn pull_image(
        &self,
        image: &str,
        credentials: Option<(String, String)>,
    ) -> Result<()> {
        use bollard::auth::DockerCredentials;
        use bollard::image::CreateImageOptions;
        use futures::TryStreamExt as _;

        let creds = credentials.map(|(username, password)| DockerCredentials {
            username: Some(username),
            password: Some(password),
            ..Default::default()
        });
        let opts = CreateImageOptions {
            from_image: image,
            ..Default::default()
        };
        let mut stream = self.inner.create_image(Some(opts), None, creds);
        while let Some(info) = stream
            .try_next()
            .await
            .with_context(|| format!("error pulling image '{image}'"))?
        {
            if let Some(status) = &info.status {
                tracing::trace!(image, status, "pull progress");
            }
            if let Some(err) = &info.error {
                anyhow::bail!("pull failed for '{}': {}", image, err);
            }
        }
        Ok(())
    }

    pub async fn stop_container(&self, id: &str, timeout_secs: i64) -> Result<()> {
        use bollard::container::StopContainerOptions;
        match self
            .inner
            .stop_container(id, Some(StopContainerOptions { t: timeout_secs }))
            .await
        {
            Ok(()) => Ok(()),
            // 304 = container already stopped; treat as success
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 304, ..
            }) => Ok(()),
            Err(e) => {
                Err(anyhow::Error::from(e).context(format!("failed to stop container '{id}'")))
            }
        }
    }

    pub async fn remove_container(&self, id: &str) -> Result<()> {
        use bollard::container::RemoveContainerOptions;
        self.inner
            .remove_container(
                id,
                Some(RemoveContainerOptions {
                    force: false,
                    v: false,
                    link: false,
                }),
            )
            .await
            .with_context(|| format!("failed to remove container '{id}'"))
    }

    pub async fn create_container(
        &self,
        name: &str,
        config: bollard::container::Config<String>,
    ) -> Result<String> {
        use bollard::container::CreateContainerOptions;
        let resp = self
            .inner
            .create_container(
                Some(CreateContainerOptions {
                    name,
                    platform: None,
                }),
                config,
            )
            .await
            .with_context(|| format!("failed to create container '{name}'"))?;
        Ok(resp.id)
    }

    pub async fn start_container(&self, id: &str) -> Result<()> {
        self.inner
            .start_container(id, None::<bollard::container::StartContainerOptions<String>>)
            .await
            .with_context(|| format!("failed to start container '{id}'"))
    }

    pub async fn remove_image(&self, image: &str) -> Result<()> {
        use bollard::image::RemoveImageOptions;
        self.inner
            .remove_image(
                image,
                Some(RemoveImageOptions {
                    force: false,
                    noprune: false,
                }),
                None,
            )
            .await
            .with_context(|| format!("failed to remove image '{image}'"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_socket_by_scheme() {
        assert_eq!(
            connection_type("unix:///var/run/docker.sock", false),
            ConnectionType::Socket
        );
    }

    #[test]
    fn unix_socket_by_path() {
        assert_eq!(
            connection_type("/var/run/docker.sock", false),
            ConnectionType::Socket
        );
    }

    #[test]
    fn npipe_is_socket() {
        assert_eq!(
            connection_type("npipe:////./pipe/docker_engine", false),
            ConnectionType::Socket
        );
    }

    #[test]
    fn tcp_without_tls_is_http() {
        assert_eq!(
            connection_type("tcp://localhost:2375", false),
            ConnectionType::Http
        );
    }

    #[test]
    fn http_scheme_is_http() {
        assert_eq!(
            connection_type("http://localhost:2375", false),
            ConnectionType::Http
        );
    }

    #[test]
    fn tls_verify_flag_forces_https() {
        assert_eq!(
            connection_type("tcp://localhost:2376", true),
            ConnectionType::Https
        );
    }

    #[test]
    fn https_scheme_selects_https() {
        assert_eq!(
            connection_type("https://localhost:2376", false),
            ConnectionType::Https
        );
    }

    #[test]
    fn parse_api_version_standard() {
        let v = parse_api_version("1.44").unwrap();
        assert_eq!(v.major_version, 1);
        assert_eq!(v.minor_version, 44);
    }

    #[test]
    fn parse_api_version_with_v_prefix() {
        let v = parse_api_version("v1.44").unwrap();
        assert_eq!(v.major_version, 1);
        assert_eq!(v.minor_version, 44);
    }

    #[test]
    fn parse_api_version_invalid_format() {
        assert!(parse_api_version("not-a-version").is_err());
    }

    #[test]
    fn parse_api_version_missing_minor() {
        assert!(parse_api_version("1").is_err());
    }

    // ── ContainerState ────────────────────────────────────────────────────────

    #[test]
    fn container_state_known_variants() {
        assert_eq!(ContainerState::from_str("created"), ContainerState::Created);
        assert_eq!(
            ContainerState::from_str("restarting"),
            ContainerState::Restarting
        );
        assert_eq!(ContainerState::from_str("running"), ContainerState::Running);
        assert_eq!(
            ContainerState::from_str("removing"),
            ContainerState::Removing
        );
        assert_eq!(ContainerState::from_str("paused"), ContainerState::Paused);
        assert_eq!(ContainerState::from_str("exited"), ContainerState::Exited);
        assert_eq!(ContainerState::from_str("dead"), ContainerState::Dead);
    }

    #[test]
    fn container_state_unknown_preserved() {
        assert_eq!(
            ContainerState::from_str("something-new"),
            ContainerState::Unknown("something-new".to_string())
        );
    }

    #[test]
    fn container_state_display() {
        assert_eq!(ContainerState::Running.to_string(), "running");
        assert_eq!(ContainerState::Exited.to_string(), "exited");
        assert_eq!(
            ContainerState::Unknown("weird".to_string()).to_string(),
            "weird"
        );
    }

    // ── SaurronLabels ─────────────────────────────────────────────────────────

    fn labels(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn saurron_labels_empty_map_gives_defaults() {
        assert_eq!(
            SaurronLabels::from_labels(&HashMap::new()),
            SaurronLabels::default()
        );
    }

    #[test]
    fn saurron_labels_enable_true() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.enable", "true")]));
        assert_eq!(l.enable, Some(true));
    }

    #[test]
    fn saurron_labels_enable_false() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.enable", "false")]));
        assert_eq!(l.enable, Some(false));
    }

    #[test]
    fn saurron_labels_enable_case_insensitive() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.enable", "TRUE")]));
        assert_eq!(l.enable, Some(true));
        let l = SaurronLabels::from_labels(&labels(&[("saurron.enable", "False")]));
        assert_eq!(l.enable, Some(false));
    }

    #[test]
    fn saurron_labels_enable_invalid_value_is_none() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.enable", "yes")]));
        assert_eq!(l.enable, None);
        let l = SaurronLabels::from_labels(&labels(&[("saurron.enable", "1")]));
        assert_eq!(l.enable, None);
        let l = SaurronLabels::from_labels(&labels(&[("saurron.enable", "")]));
        assert_eq!(l.enable, None);
    }

    #[test]
    fn saurron_labels_scope() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.scope", "production")]));
        assert_eq!(l.scope, Some("production".to_string()));
    }

    #[test]
    fn saurron_labels_scope_empty_is_none() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.scope", "")]));
        assert_eq!(l.scope, None);
    }

    #[test]
    fn saurron_labels_depends_on_multiple() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.depends-on", "db,redis")]));
        assert_eq!(l.depends_on, vec!["db", "redis"]);
    }

    #[test]
    fn saurron_labels_depends_on_trims_whitespace() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.depends-on", "db, redis , cache")]));
        assert_eq!(l.depends_on, vec!["db", "redis", "cache"]);
    }

    #[test]
    fn saurron_labels_depends_on_empty_string_gives_empty_vec() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.depends-on", "")]));
        assert_eq!(l.depends_on, Vec::<String>::new());
    }

    #[test]
    fn saurron_labels_depends_on_sparse_commas_filtered() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.depends-on", ",db,,redis,")]));
        assert_eq!(l.depends_on, vec!["db", "redis"]);
    }

    #[test]
    fn saurron_labels_image_tag() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.image-tag", "v1.2.3")]));
        assert_eq!(l.image_tag, Some("v1.2.3".to_string()));
    }

    #[test]
    fn saurron_labels_image_tag_empty_is_none() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.image-tag", "")]));
        assert_eq!(l.image_tag, None);
    }

    #[test]
    fn saurron_labels_semver_pre_release_true() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.semver-pre-release", "true")]));
        assert_eq!(l.semver_pre_release, Some(true));
    }

    #[test]
    fn saurron_labels_semver_pre_release_false() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.semver-pre-release", "false")]));
        assert_eq!(l.semver_pre_release, Some(false));
    }

    #[test]
    fn saurron_labels_semver_pre_release_invalid_is_none() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.semver-pre-release", "yes")]));
        assert_eq!(l.semver_pre_release, None);
    }

    #[test]
    fn saurron_labels_non_semver_strategy_skip() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.non-semver-strategy", "skip")]));
        assert_eq!(l.non_semver_strategy, Some("skip".to_string()));
    }

    #[test]
    fn saurron_labels_non_semver_strategy_digest() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.non-semver-strategy", "digest")]));
        assert_eq!(l.non_semver_strategy, Some("digest".to_string()));
    }

    #[test]
    fn saurron_labels_non_semver_strategy_empty_is_none() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.non-semver-strategy", "")]));
        assert_eq!(l.non_semver_strategy, None);
    }

    #[test]
    fn saurron_labels_monitor_only_true() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.monitor-only", "true")]));
        assert_eq!(l.monitor_only, Some(true));
    }

    #[test]
    fn saurron_labels_monitor_only_false() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.monitor-only", "false")]));
        assert_eq!(l.monitor_only, Some(false));
    }

    #[test]
    fn saurron_labels_monitor_only_invalid_is_none() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.monitor-only", "yes")]));
        assert_eq!(l.monitor_only, None);
    }

    #[test]
    fn saurron_labels_no_pull_true() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.no-pull", "true")]));
        assert_eq!(l.no_pull, Some(true));
    }

    #[test]
    fn saurron_labels_stop_signal_set() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.stop-signal", "SIGHUP")]));
        assert_eq!(l.stop_signal, Some("SIGHUP".to_string()));
    }

    #[test]
    fn saurron_labels_stop_signal_empty_is_none() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.stop-signal", "")]));
        assert_eq!(l.stop_signal, None);
    }

    #[test]
    fn saurron_labels_stop_timeout_set() {
        let l = SaurronLabels::from_labels(&labels(&[("saurron.stop-timeout", "30s")]));
        assert_eq!(l.stop_timeout, Some("30s".to_string()));
    }

    #[test]
    fn saurron_labels_new_fields_default_none() {
        let l = SaurronLabels::default();
        assert_eq!(l.monitor_only, None);
        assert_eq!(l.no_pull, None);
        assert_eq!(l.stop_signal, None);
        assert_eq!(l.stop_timeout, None);
    }

    #[test]
    fn saurron_labels_unknown_saurron_labels_ignored() {
        let l = SaurronLabels::from_labels(&labels(&[
            ("saurron.enable", "true"),
            ("saurron.future-feature", "somevalue"),
            ("com.example.app", "myapp"),
        ]));
        assert_eq!(l.enable, Some(true));
        assert_eq!(l.scope, None);
    }

    #[test]
    fn container_info_saurron_labels_convenience() {
        let info = ContainerInfo {
            id: "abc123".to_string(),
            name: "mycontainer".to_string(),
            image: "nginx:latest".to_string(),
            image_id: "sha256:abc".to_string(),
            state: ContainerState::Running,
            labels: labels(&[("saurron.enable", "true"), ("saurron.image-tag", "stable")]),
        };
        let sl = info.saurron_labels();
        assert_eq!(sl.enable, Some(true));
        assert_eq!(sl.image_tag, Some("stable".to_string()));
    }

    // ── local_image_info_from_inspect ─────────────────────────────────────────

    fn make_inspect(
        repo_tags: Option<Vec<&str>>,
        repo_digests: Option<Vec<&str>>,
    ) -> bollard::models::ImageInspect {
        bollard::models::ImageInspect {
            repo_tags: repo_tags.map(|v| v.into_iter().map(String::from).collect()),
            repo_digests: repo_digests.map(|v| v.into_iter().map(String::from).collect()),
            ..Default::default()
        }
    }

    #[test]
    fn image_info_name_from_first_repo_tag() {
        let inspect = make_inspect(
            Some(vec!["postgres:15", "postgres:latest"]),
            Some(vec![
                "postgres@sha256:6eed15406dbba206cb1260528a3354d80d2522cab068cb9ad7a1ede5ac90e6f6",
            ]),
        );
        let info = local_image_info_from_inspect(&inspect);
        assert_eq!(info.name, Some("postgres:15".to_string()));
        assert_eq!(
            info.digest,
            Some(
                "sha256:6eed15406dbba206cb1260528a3354d80d2522cab068cb9ad7a1ede5ac90e6f6"
                    .to_string()
            )
        );
    }

    #[test]
    fn image_info_empty_repo_tags_gives_none_name() {
        let inspect = make_inspect(Some(vec![]), Some(vec!["postgres@sha256:abc"]));
        let info = local_image_info_from_inspect(&inspect);
        assert_eq!(info.name, None);
        assert_eq!(info.digest, Some("sha256:abc".to_string()));
    }

    #[test]
    fn image_info_none_fields_give_none() {
        let inspect = make_inspect(None, None);
        assert_eq!(
            local_image_info_from_inspect(&inspect),
            LocalImageInfo::default()
        );
    }

    #[test]
    fn image_info_digest_extracted_after_at_sign() {
        let inspect = make_inspect(
            Some(vec!["nginx:latest"]),
            Some(vec!["nginx@sha256:deadbeef"]),
        );
        let info = local_image_info_from_inspect(&inspect);
        assert_eq!(info.digest, Some("sha256:deadbeef".to_string()));
    }

    // ── ContainerSelector ─────────────────────────────────────────────────────

    fn make_container(name: &str, state: ContainerState, ls: &[(&str, &str)]) -> ContainerInfo {
        ContainerInfo {
            id: format!("{name}_id"),
            name: name.to_string(),
            image: format!("{name}:latest"),
            image_id: "sha256:abc".to_string(),
            state,
            labels: labels(ls),
        }
    }

    fn running(name: &str, ls: &[(&str, &str)]) -> ContainerInfo {
        make_container(name, ContainerState::Running, ls)
    }

    fn opt_out() -> ContainerSelector {
        ContainerSelector::new(false, false, &[], &[], false, false)
    }

    fn opt_in() -> ContainerSelector {
        ContainerSelector::new(true, false, &[], &[], false, false)
    }

    // State filter

    #[test]
    fn state_filter_default_is_running_only() {
        assert_eq!(opt_out().state_filter(), vec!["running"]);
    }

    #[test]
    fn state_filter_include_restarting() {
        let sel = ContainerSelector::new(false, false, &[], &[], true, false);
        assert_eq!(sel.state_filter(), vec!["running", "restarting"]);
    }

    #[test]
    fn state_filter_revive_stopped() {
        let sel = ContainerSelector::new(false, false, &[], &[], false, true);
        assert_eq!(sel.state_filter(), vec!["running", "exited", "created"]);
    }

    #[test]
    fn state_filter_both_flags() {
        let sel = ContainerSelector::new(false, false, &[], &[], true, true);
        assert_eq!(
            sel.state_filter(),
            vec!["running", "restarting", "exited", "created"]
        );
    }

    // Opt-out selection

    #[test]
    fn opt_out_no_labels_included() {
        assert!(opt_out().is_selected(&running("app", &[])));
    }

    #[test]
    fn opt_out_enable_true_included() {
        assert!(opt_out().is_selected(&running("app", &[("saurron.enable", "true")])));
    }

    #[test]
    fn opt_out_enable_false_excluded() {
        assert!(!opt_out().is_selected(&running("app", &[("saurron.enable", "false")])));
    }

    // Opt-in selection

    #[test]
    fn opt_in_no_labels_excluded() {
        assert!(!opt_in().is_selected(&running("app", &[])));
    }

    #[test]
    fn opt_in_enable_true_included() {
        assert!(opt_in().is_selected(&running("app", &[("saurron.enable", "true")])));
    }

    #[test]
    fn opt_in_enable_false_excluded() {
        assert!(!opt_in().is_selected(&running("app", &[("saurron.enable", "false")])));
    }

    // disable_containers

    #[test]
    fn disabled_name_excluded_in_opt_out() {
        let sel = ContainerSelector::new(false, false, &["app".to_string()], &[], false, false);
        assert!(!sel.is_selected(&running("app", &[("saurron.enable", "true")])));
    }

    #[test]
    fn disabled_name_excluded_in_opt_in() {
        let sel = ContainerSelector::new(true, false, &["app".to_string()], &[], false, false);
        assert!(!sel.is_selected(&running("app", &[("saurron.enable", "true")])));
    }

    #[test]
    fn non_disabled_name_unaffected() {
        let sel = ContainerSelector::new(false, false, &["other".to_string()], &[], false, false);
        assert!(sel.is_selected(&running("app", &[])));
    }

    // global_takes_precedence

    #[test]
    fn global_precedence_overrides_per_container_disable() {
        let sel = ContainerSelector::new(false, true, &[], &[], false, false);
        assert!(sel.is_selected(&running("app", &[("saurron.enable", "false")])));
    }

    #[test]
    fn global_precedence_disable_containers_still_excluded() {
        let sel = ContainerSelector::new(false, true, &["app".to_string()], &[], false, false);
        assert!(!sel.is_selected(&running("app", &[])));
    }

    #[test]
    fn global_precedence_no_label_still_included() {
        let sel = ContainerSelector::new(false, true, &[], &[], false, false);
        assert!(sel.is_selected(&running("app", &[])));
    }

    // select()

    #[test]
    fn select_returns_only_matching_containers() {
        let containers = vec![
            running("enabled", &[("saurron.enable", "true")]),
            running("unlabelled", &[]),
            running("disabled", &[("saurron.enable", "false")]),
        ];
        let result = opt_out().select(&containers);
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|c| c.name == "enabled"));
        assert!(result.iter().any(|c| c.name == "unlabelled"));
    }

    #[test]
    fn select_empty_input_returns_empty() {
        assert!(opt_out().select(&[]).is_empty());
    }

    #[test]
    fn select_opt_in_filters_to_enabled_only() {
        let containers = vec![
            running("a", &[("saurron.enable", "true")]),
            running("b", &[]),
            running("c", &[("saurron.enable", "true")]),
        ];
        let result = opt_in().select(&containers);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|c| c.name == "a" || c.name == "c"));
    }

    // allowed_names (--containers)

    #[test]
    fn allowed_names_empty_slice_means_no_restriction() {
        let sel = ContainerSelector::new(false, false, &[], &[], false, false);
        assert!(sel.allowed_names.is_none());
        assert!(sel.is_selected(&running("any", &[])));
    }

    #[test]
    fn allowed_names_matching_container_included() {
        let sel = ContainerSelector::new(
            false,
            false,
            &[],
            &["foo".to_string(), "bar".to_string()],
            false,
            false,
        );
        assert!(sel.is_selected(&running("foo", &[])));
        assert!(sel.is_selected(&running("bar", &[])));
    }

    #[test]
    fn allowed_names_non_matching_container_excluded() {
        let sel = ContainerSelector::new(false, false, &[], &["foo".to_string()], false, false);
        assert!(!sel.is_selected(&running("other", &[])));
    }

    #[test]
    fn allowed_names_disable_containers_still_excludes() {
        let sel = ContainerSelector::new(
            false,
            false,
            &["foo".to_string()],
            &["foo".to_string()],
            false,
            false,
        );
        // foo is in allowed_names but also in disabled_names — disabled wins
        assert!(!sel.is_selected(&running("foo", &[])));
    }

    #[test]
    fn allowed_names_with_label_enable_still_requires_label() {
        let sel = ContainerSelector::new(true, false, &[], &["foo".to_string()], false, false);
        // foo is in allowed_names but label_enable mode requires saurron.enable=true
        assert!(!sel.is_selected(&running("foo", &[])));
        assert!(sel.is_selected(&running("foo", &[("saurron.enable", "true")])));
    }

    #[test]
    fn allowed_names_select_filters_to_listed_containers() {
        let containers = vec![
            running("foo", &[]),
            running("bar", &[]),
            running("baz", &[]),
        ];
        let sel = ContainerSelector::new(
            false,
            false,
            &[],
            &["foo".to_string(), "baz".to_string()],
            false,
            false,
        );
        let result = sel.select(&containers);
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|c| c.name == "foo"));
        assert!(result.iter().any(|c| c.name == "baz"));
        assert!(!result.iter().any(|c| c.name == "bar"));
    }
}
