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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SaurronLabels {
    pub enable: Option<bool>,
    pub scope: Option<String>,
    pub depends_on: Vec<String>,
    pub image_tag: Option<String>,
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
    include_restarting: bool,
    revive_stopped: bool,
}

impl ContainerSelector {
    pub fn new(
        label_enable: bool,
        global_takes_precedence: bool,
        disable_containers: &[String],
        include_restarting: bool,
        revive_stopped: bool,
    ) -> Self {
        Self {
            label_enable,
            global_takes_precedence,
            disabled_names: disable_containers.iter().cloned().collect(),
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
        ContainerSelector::new(false, false, &[], false, false)
    }

    fn opt_in() -> ContainerSelector {
        ContainerSelector::new(true, false, &[], false, false)
    }

    // State filter

    #[test]
    fn state_filter_default_is_running_only() {
        assert_eq!(opt_out().state_filter(), vec!["running"]);
    }

    #[test]
    fn state_filter_include_restarting() {
        let sel = ContainerSelector::new(false, false, &[], true, false);
        assert_eq!(sel.state_filter(), vec!["running", "restarting"]);
    }

    #[test]
    fn state_filter_revive_stopped() {
        let sel = ContainerSelector::new(false, false, &[], false, true);
        assert_eq!(sel.state_filter(), vec!["running", "exited", "created"]);
    }

    #[test]
    fn state_filter_both_flags() {
        let sel = ContainerSelector::new(false, false, &[], true, true);
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
        let sel = ContainerSelector::new(false, false, &["app".to_string()], false, false);
        assert!(!sel.is_selected(&running("app", &[("saurron.enable", "true")])));
    }

    #[test]
    fn disabled_name_excluded_in_opt_in() {
        let sel = ContainerSelector::new(true, false, &["app".to_string()], false, false);
        assert!(!sel.is_selected(&running("app", &[("saurron.enable", "true")])));
    }

    #[test]
    fn non_disabled_name_unaffected() {
        let sel = ContainerSelector::new(false, false, &["other".to_string()], false, false);
        assert!(sel.is_selected(&running("app", &[])));
    }

    // global_takes_precedence

    #[test]
    fn global_precedence_overrides_per_container_disable() {
        let sel = ContainerSelector::new(false, true, &[], false, false);
        assert!(sel.is_selected(&running("app", &[("saurron.enable", "false")])));
    }

    #[test]
    fn global_precedence_disable_containers_still_excluded() {
        let sel = ContainerSelector::new(false, true, &["app".to_string()], false, false);
        assert!(!sel.is_selected(&running("app", &[])));
    }

    #[test]
    fn global_precedence_no_label_still_included() {
        let sel = ContainerSelector::new(false, true, &[], false, false);
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
}
