use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::Result;
use tracing::{debug, info, warn};

use crate::{audit, config, docker, registry};

// ── Duration parser ───────────────────────────────────────────────────────────

/// Parse a duration string of the form `<N><unit>` where unit is `s`, `m`, or `h`.
/// A bare integer is treated as seconds.
pub fn parse_duration_secs(s: &str) -> Result<u64> {
    let s = s.trim();
    if s.is_empty() {
        anyhow::bail!("empty duration string");
    }
    let (num_part, multiplier) = if let Some(n) = s.strip_suffix('s') {
        (n, 1u64)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 60u64)
    } else if let Some(n) = s.strip_suffix('h') {
        (n, 3600u64)
    } else {
        (s, 1u64)
    };
    let n: u64 = num_part
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid duration '{s}'"))?;
    Ok(n * multiplier)
}

// ── ContainerRunConfig ────────────────────────────────────────────────────────

/// Captured container configuration sufficient to recreate it with a new image.
#[derive(Debug, Clone)]
pub struct ContainerRunConfig {
    // from ContainerInspectResponse.config
    pub hostname: Option<String>,
    pub domainname: Option<String>,
    pub user: Option<String>,
    pub env: Option<Vec<String>>,
    pub cmd: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub tty: Option<bool>,
    pub open_stdin: Option<bool>,
    pub stop_signal: Option<String>,
    pub labels: Option<HashMap<String, String>>,
    pub exposed_ports: Option<HashMap<String, HashMap<(), ()>>>,
    // from ContainerInspectResponse.host_config
    pub binds: Option<Vec<String>>,
    pub volumes_from: Option<Vec<String>>,
    pub port_bindings: Option<bollard::models::PortMap>,
    pub restart_policy: Option<bollard::models::RestartPolicy>,
    pub network_mode: Option<String>,
    pub links: Option<Vec<String>>,
    pub extra_hosts: Option<Vec<String>>,
    pub cap_add: Option<Vec<String>>,
    pub cap_drop: Option<Vec<String>>,
    pub privileged: Option<bool>,
    pub devices: Option<Vec<bollard::models::DeviceMapping>>,
    pub log_config: Option<bollard::models::HostConfigLogConfig>,
    pub shm_size: Option<i64>,
    pub ulimits: Option<Vec<bollard::models::ResourcesUlimits>>,
    pub init: Option<bool>,
    // from ContainerInspectResponse.network_settings.networks
    pub networks: Option<HashMap<String, bollard::models::EndpointSettings>>,
}

fn extract_run_config(inspect: &bollard::models::ContainerInspectResponse) -> ContainerRunConfig {
    let cfg = inspect.config.as_ref();
    let hc = inspect.host_config.as_ref();
    let ns = inspect.network_settings.as_ref();

    ContainerRunConfig {
        hostname: cfg.and_then(|c| c.hostname.clone()),
        domainname: cfg.and_then(|c| c.domainname.clone()),
        user: cfg.and_then(|c| c.user.clone()),
        env: cfg.and_then(|c| c.env.clone()),
        cmd: cfg.and_then(|c| c.cmd.clone()),
        entrypoint: cfg.and_then(|c| c.entrypoint.clone()),
        working_dir: cfg.and_then(|c| c.working_dir.clone()),
        tty: cfg.and_then(|c| c.tty),
        open_stdin: cfg.and_then(|c| c.open_stdin),
        stop_signal: cfg.and_then(|c| c.stop_signal.clone()),
        labels: cfg.and_then(|c| c.labels.clone()),
        exposed_ports: cfg.and_then(|c| c.exposed_ports.clone()),
        binds: hc.and_then(|h| h.binds.clone()),
        volumes_from: hc.and_then(|h| h.volumes_from.clone()),
        port_bindings: hc.and_then(|h| h.port_bindings.clone()),
        restart_policy: hc.and_then(|h| h.restart_policy.clone()),
        network_mode: hc.and_then(|h| h.network_mode.clone()),
        links: hc.and_then(|h| h.links.clone()),
        extra_hosts: hc.and_then(|h| h.extra_hosts.clone()),
        cap_add: hc.and_then(|h| h.cap_add.clone()),
        cap_drop: hc.and_then(|h| h.cap_drop.clone()),
        privileged: hc.and_then(|h| h.privileged),
        devices: hc.and_then(|h| h.devices.clone()),
        log_config: hc.and_then(|h| h.log_config.clone()),
        shm_size: hc.and_then(|h| h.shm_size),
        ulimits: hc.and_then(|h| h.ulimits.clone()),
        init: hc.and_then(|h| h.init),
        networks: ns.and_then(|n| n.networks.clone()),
    }
}

fn build_create_config(
    run_cfg: &ContainerRunConfig,
    new_image: &str,
    stop_signal_override: Option<&str>,
) -> bollard::container::Config<String> {
    use bollard::container::NetworkingConfig;

    let networking_config = run_cfg.networks.as_ref().map(|nets| NetworkingConfig {
        endpoints_config: nets.clone(),
    });

    let host_config = Some(bollard::models::HostConfig {
        binds: run_cfg.binds.clone(),
        volumes_from: run_cfg.volumes_from.clone(),
        port_bindings: run_cfg.port_bindings.clone(),
        restart_policy: run_cfg.restart_policy.clone(),
        network_mode: run_cfg.network_mode.clone(),
        links: run_cfg.links.clone(),
        extra_hosts: run_cfg.extra_hosts.clone(),
        cap_add: run_cfg.cap_add.clone(),
        cap_drop: run_cfg.cap_drop.clone(),
        privileged: run_cfg.privileged,
        devices: run_cfg.devices.clone(),
        log_config: run_cfg.log_config.clone(),
        shm_size: run_cfg.shm_size,
        ulimits: run_cfg.ulimits.clone(),
        init: run_cfg.init,
        ..Default::default()
    });

    let effective_stop_signal = stop_signal_override
        .map(|s| s.to_string())
        .or_else(|| run_cfg.stop_signal.clone());

    bollard::container::Config {
        hostname: run_cfg.hostname.clone(),
        domainname: run_cfg.domainname.clone(),
        user: run_cfg.user.clone(),
        env: run_cfg.env.clone(),
        cmd: run_cfg.cmd.clone(),
        entrypoint: run_cfg.entrypoint.clone(),
        working_dir: run_cfg.working_dir.clone(),
        tty: run_cfg.tty,
        open_stdin: run_cfg.open_stdin,
        stop_signal: effective_stop_signal,
        labels: run_cfg.labels.clone(),
        exposed_ports: run_cfg.exposed_ports.clone(),
        image: Some(new_image.to_string()),
        host_config,
        networking_config,
        ..Default::default()
    }
}

// ── Dependency graph + topological sort ──────────────────────────────────────

fn parse_link_target(link: &str) -> Option<String> {
    // Docker link format: "/real_target:/container_name/alias" or "target:alias"
    let link = link.trim_start_matches('/');
    let target = link.split(':').next()?;
    let target = target.trim_start_matches('/');
    if target.is_empty() {
        None
    } else {
        Some(target.to_string())
    }
}

/// Build dep_graph[name] = [names this container depends on].
/// Sources: saurron.depends-on label, Docker --link, network_mode: container:<name>.
/// Only names present in the container set are included.
fn build_dependency_graph(
    containers: &[docker::ContainerInfo],
    inspect_map: &HashMap<String, bollard::models::ContainerInspectResponse>,
) -> HashMap<String, Vec<String>> {
    let name_set: HashSet<&str> = containers.iter().map(|c| c.name.as_str()).collect();
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();

    for c in containers {
        let mut deps: Vec<String> = Vec::new();

        // 1. saurron.depends-on label
        for dep in &c.saurron_labels().depends_on {
            if name_set.contains(dep.as_str()) {
                deps.push(dep.clone());
            }
        }

        if let Some(inspect) = inspect_map.get(&c.name) {
            if let Some(hc) = &inspect.host_config {
                // 2. Docker --link
                for link in hc.links.iter().flatten() {
                    if let Some(target) = parse_link_target(link) {
                        if name_set.contains(target.as_str()) {
                            deps.push(target);
                        }
                    }
                }
                // 3. network_mode: container:<name>
                if let Some(nm) = &hc.network_mode {
                    if let Some(dep_name) = nm.strip_prefix("container:") {
                        if name_set.contains(dep_name) {
                            deps.push(dep_name.to_string());
                        }
                    }
                }
            }
        }

        deps.dedup();
        graph.insert(c.name.clone(), deps);
    }
    graph
}

/// Sort containers leaves-first: containers with no dependents update before
/// containers that others depend on. Uses Kahn's algorithm on the reverse
/// dependency graph. Cycles are appended at the end with a warning.
fn topological_sort(
    containers: &[docker::ContainerInfo],
    dep_graph: &HashMap<String, Vec<String>>,
) -> Vec<docker::ContainerInfo> {
    // rev_in_degree[X] = number of containers that depend on X
    let mut rev_in_degree: HashMap<&str, usize> = HashMap::new();
    for c in containers {
        rev_in_degree.entry(c.name.as_str()).or_insert(0);
    }
    for c in containers {
        for dep in dep_graph.get(&c.name).into_iter().flatten() {
            *rev_in_degree.entry(dep.as_str()).or_insert(0) += 1;
        }
    }

    // Start with containers that nobody depends on (leaves)
    let mut queue: VecDeque<&str> = rev_in_degree
        .iter()
        .filter(|&(_, &d)| d == 0)
        .map(|(&name, _)| name)
        .collect();

    let name_to_info: HashMap<&str, &docker::ContainerInfo> =
        containers.iter().map(|c| (c.name.as_str(), c)).collect();

    let mut result: Vec<docker::ContainerInfo> = Vec::with_capacity(containers.len());

    while let Some(name) = queue.pop_front() {
        if let Some(info) = name_to_info.get(name) {
            result.push((*info).clone());
        }
        // After committing to updating this container, its dependencies have
        // one fewer pending dependent; enqueue any that are now unblocked.
        for dep in dep_graph.get(name).into_iter().flatten() {
            let deg = rev_in_degree.get_mut(dep.as_str()).unwrap();
            *deg -= 1;
            if *deg == 0 {
                queue.push_back(dep.as_str());
            }
        }
    }

    // Append any cycle members in original order
    let in_result: HashSet<String> = result.iter().map(|c| c.name.clone()).collect();
    for c in containers {
        if !in_result.contains(&c.name) {
            warn!(container = %c.name, "dependency cycle detected; updating in original order");
            result.push(c.clone());
        }
    }

    result
}

// ── Result types ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum UpdateResult {
    UpToDate,
    Skipped(String),
    Updated {
        old_image: String,
        old_digest: String,
        new_image: String,
        new_digest: String,
    },
    Failed(anyhow::Error),
}

#[derive(Debug, Default)]
pub struct SessionReport {
    pub updated: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<String>,
    pub up_to_date: usize,
}

impl SessionReport {
    pub fn record(&mut self, name: &str, result: &UpdateResult) {
        match result {
            UpdateResult::Updated { .. } => self.updated.push(name.to_string()),
            UpdateResult::Skipped(_) => self.skipped.push(name.to_string()),
            UpdateResult::Failed(_) => self.failed.push(name.to_string()),
            UpdateResult::UpToDate => self.up_to_date += 1,
        }
    }
}

// ── Override resolution helpers ───────────────────────────────────────────────

fn resolve_bool_override(global: bool, global_takes_precedence: bool, label: Option<bool>) -> bool {
    if global_takes_precedence {
        global
    } else {
        label.unwrap_or(global)
    }
}

// ── UpdateEngine ──────────────────────────────────────────────────────────────

pub struct UpdateEngine<'a> {
    docker: &'a docker::DockerClient,
    registry: &'a registry::RegistryClient,
    config: &'a config::Config,
    credentials: Option<(String, String)>,
}

impl<'a> UpdateEngine<'a> {
    pub fn new(
        docker: &'a docker::DockerClient,
        registry: &'a registry::RegistryClient,
        config: &'a config::Config,
    ) -> Self {
        let credentials = match (&config.registry_username, &config.registry_password) {
            (Some(u), Some(p)) => Some((u.clone(), p.clone())),
            _ => None,
        };
        Self {
            docker,
            registry,
            config,
            credentials,
        }
    }

    pub async fn run_cycle(&self, containers: &[docker::ContainerInfo]) -> SessionReport {
        let mut report = SessionReport::default();

        // Phase A: scan all containers for staleness
        let mut stale: Vec<(docker::ContainerInfo, registry::StaleInfo)> = Vec::new();
        for container in containers {
            match self.check_freshness(container).await {
                registry::FreshnessResult::UpToDate => {
                    debug!(container = %container.name, "image up to date");
                    report.up_to_date += 1;
                }
                registry::FreshnessResult::Stale(info) => {
                    info!(
                        container = %container.name,
                        new_image = %info.new_image,
                        "stale image detected"
                    );
                    stale.push((container.clone(), info));
                }
                registry::FreshnessResult::Skipped(reason) => {
                    info!(container = %container.name, reason, "freshness check skipped");
                    report.record(&container.name, &UpdateResult::Skipped(reason));
                }
                registry::FreshnessResult::Error(reason) => {
                    warn!(container = %container.name, reason, "freshness check failed");
                    report.record(&container.name, &UpdateResult::Skipped(reason));
                }
            }
        }

        if stale.is_empty() {
            info!(
                total = containers.len(),
                "All containers up to date"
            );
            return report;
        }

        // Phase B: inspect stale containers to capture full run config
        let mut inspect_map: HashMap<String, bollard::models::ContainerInspectResponse> =
            HashMap::new();
        let mut inspected_stale: Vec<(docker::ContainerInfo, registry::StaleInfo)> = Vec::new();
        for (c, info) in stale {
            match self.docker.inspect_container(&c.id).await {
                Ok(resp) => {
                    inspect_map.insert(c.name.clone(), resp);
                    inspected_stale.push((c, info));
                }
                Err(e) => {
                    warn!(container = %c.name, error = %e, "inspect failed; skipping update");
                    report.record(&c.name, &UpdateResult::Failed(e));
                }
            }
        }

        // Phase C: topological sort (leaves first)
        let stale_containers: Vec<docker::ContainerInfo> =
            inspected_stale.iter().map(|(c, _)| c.clone()).collect();
        let dep_graph = build_dependency_graph(&stale_containers, &inspect_map);
        let ordered = topological_sort(&stale_containers, &dep_graph);

        let stale_map: HashMap<String, registry::StaleInfo> = inspected_stale
            .into_iter()
            .map(|(c, info)| (c.name, info))
            .collect();

        // Phase D: update each stale container in dependency order
        for container in &ordered {
            let Some(stale_info) = stale_map.get(&container.name) else {
                continue;
            };
            let Some(inspect) = inspect_map.get(&container.name) else {
                continue;
            };
            let result = self.update_one(container, stale_info, inspect).await;
            if let UpdateResult::Failed(ref e) = result {
                warn!(container = %container.name, error = %e, "update failed");
            }
            report.record(&container.name, &result);
        }

        // Phase E: session summary
        info!(
            updated = report.updated.len(),
            skipped = report.skipped.len(),
            failed = report.failed.len(),
            up_to_date = report.up_to_date,
            "Update cycle complete"
        );

        report
    }

    async fn check_freshness(
        &self,
        container: &docker::ContainerInfo,
    ) -> registry::FreshnessResult {
        let image_info = match self.docker.get_local_image_info(&container.image).await {
            Ok(info) => info,
            Err(e) => {
                warn!(
                    container = %container.name,
                    image = %container.image,
                    error = %e,
                    "failed to inspect local image; treating as no local digest"
                );
                docker::LocalImageInfo::default()
            }
        };

        let image_for_check = image_info.name.as_deref().unwrap_or(&container.image);
        let labels = container.saurron_labels();
        let allow_pre = labels.semver_pre_release.unwrap_or(false);
        let strategy = labels
            .non_semver_strategy
            .as_deref()
            .map(registry::parse_non_semver_strategy)
            .unwrap_or_default();

        self.registry
            .check_freshness(
                image_for_check,
                image_info.digest.as_deref(),
                allow_pre,
                strategy,
            )
            .await
    }

    async fn update_one(
        &self,
        container: &docker::ContainerInfo,
        stale_info: &registry::StaleInfo,
        inspect: &bollard::models::ContainerInspectResponse,
    ) -> UpdateResult {
        let labels = container.saurron_labels();
        let cfg = self.config;

        // Resolve per-container overrides
        let effective_monitor_only =
            resolve_bool_override(cfg.monitor_only, cfg.global_takes_precedence, labels.monitor_only);
        let effective_no_pull =
            resolve_bool_override(cfg.no_pull, cfg.global_takes_precedence, labels.no_pull);
        // stop_signal: label always wins (no global stop-signal config)
        let effective_stop_signal: Option<String> = labels.stop_signal.clone();
        let stop_timeout_secs: i64 = {
            let raw = labels
                .stop_timeout
                .as_deref()
                .unwrap_or(&cfg.stop_timeout)
                .to_string();
            parse_duration_secs(&raw).unwrap_or(10) as i64
        };

        if effective_monitor_only {
            info!(
                container = %container.name,
                new_image = %stale_info.new_image,
                "monitor-only: skipping update"
            );
            return UpdateResult::Skipped("monitor-only".to_string());
        }

        let old_image = container.image.clone();
        let old_digest = stale_info.current_digest.clone();

        // Step 1: pull new image
        if !effective_no_pull {
            info!(container = %container.name, image = %stale_info.new_image, "pulling new image");
            if let Err(e) = self
                .docker
                .pull_image(&stale_info.new_image, self.credentials.clone())
                .await
            {
                return UpdateResult::Failed(
                    e.context(format!("pull failed for '{}'", stale_info.new_image)),
                );
            }
        } else {
            info!(container = %container.name, "no-pull: using cached image");
        }

        // Step 2: get new image digest for audit trail
        let new_digest = match self
            .docker
            .get_local_image_info(&stale_info.new_image)
            .await
        {
            Ok(info) => info.digest.unwrap_or_else(|| stale_info.new_digest.clone()),
            Err(_) => stale_info.new_digest.clone(),
        };

        // Step 3: extract old container run config before stopping
        let run_cfg = extract_run_config(inspect);

        // Step 4: stop old container
        info!(
            container = %container.name,
            id = %container.id,
            timeout_secs = stop_timeout_secs,
            "stopping container"
        );
        if let Err(e) = self
            .docker
            .stop_container(&container.id, stop_timeout_secs)
            .await
        {
            return UpdateResult::Failed(
                e.context(format!("failed to stop container '{}'", container.name)),
            );
        }

        // Step 5: remove old container
        if let Err(e) = self.docker.remove_container(&container.id).await {
            return UpdateResult::Failed(
                e.context(format!("failed to remove container '{}'", container.name)),
            );
        }

        // Step 6: create new container with updated image
        let create_cfg = build_create_config(
            &run_cfg,
            &stale_info.new_image,
            effective_stop_signal.as_deref(),
        );
        info!(
            container = %container.name,
            new_image = %stale_info.new_image,
            "recreating container"
        );
        let new_id = match self
            .docker
            .create_container(&container.name, create_cfg)
            .await
        {
            Ok(id) => id,
            Err(e) => {
                return UpdateResult::Failed(
                    e.context(format!("failed to create container '{}'", container.name)),
                );
            }
        };

        // Step 7: start new container
        if let Err(e) = self.docker.start_container(&new_id).await {
            return UpdateResult::Failed(
                e.context(format!("failed to start container '{}'", container.name)),
            );
        }

        // Step 8: basic startup check (Phase 5 — rollback added in Phase 6)
        let startup_timeout = parse_duration_secs(&cfg.rollback.startup_timeout).unwrap_or(30);
        let wait_secs = startup_timeout.min(10);
        tokio::time::sleep(tokio::time::Duration::from_secs(wait_secs)).await;
        match self.docker.inspect_container(&new_id).await {
            Ok(new_inspect) => {
                let running = new_inspect
                    .state
                    .as_ref()
                    .and_then(|s| s.running)
                    .unwrap_or(false);
                if !running {
                    let status = new_inspect
                        .state
                        .as_ref()
                        .and_then(|s| s.status.as_ref())
                        .map(|s| format!("{s:?}"))
                        .unwrap_or_else(|| "unknown".to_string());
                    warn!(
                        container = %container.name,
                        new_id = %new_id,
                        status,
                        "container not running after startup (rollback deferred to Phase 6)"
                    );
                } else {
                    info!(container = %container.name, new_id = %new_id, "container started successfully");
                }
            }
            Err(e) => {
                warn!(container = %container.name, error = %e, "could not verify startup state");
            }
        }

        // Step 9: audit trail
        audit::audit_update(
            &container.name,
            &new_id,
            &old_image,
            &old_digest,
            &stale_info.new_image,
            &new_digest,
        );

        // Step 10: optional old image cleanup
        if cfg.cleanup {
            info!(container = %container.name, image = %old_image, "removing old image");
            if let Err(e) = self.docker.remove_image(&old_image).await {
                warn!(
                    container = %container.name,
                    image = %old_image,
                    error = %e,
                    "old image removal failed (non-fatal)"
                );
            }
        }

        UpdateResult::Updated {
            old_image,
            old_digest,
            new_image: stale_info.new_image.clone(),
            new_digest,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_duration_secs ───────────────────────────────────────────────────

    #[test]
    fn duration_seconds() {
        assert_eq!(parse_duration_secs("10s").unwrap(), 10);
    }

    #[test]
    fn duration_minutes() {
        assert_eq!(parse_duration_secs("5m").unwrap(), 300);
    }

    #[test]
    fn duration_hours() {
        assert_eq!(parse_duration_secs("1h").unwrap(), 3600);
    }

    #[test]
    fn duration_bare_integer() {
        assert_eq!(parse_duration_secs("30").unwrap(), 30);
    }

    #[test]
    fn duration_zero() {
        assert_eq!(parse_duration_secs("0s").unwrap(), 0);
    }

    #[test]
    fn duration_empty_is_error() {
        assert!(parse_duration_secs("").is_err());
    }

    #[test]
    fn duration_non_numeric_is_error() {
        assert!(parse_duration_secs("abc").is_err());
    }

    #[test]
    fn duration_unknown_unit_is_error() {
        assert!(parse_duration_secs("5z").is_err());
    }

    // ── parse_link_target ─────────────────────────────────────────────────────

    #[test]
    fn link_target_docker_format() {
        assert_eq!(
            parse_link_target("/redis:/myapp/redis"),
            Some("redis".to_string())
        );
    }

    #[test]
    fn link_target_simple_format() {
        assert_eq!(
            parse_link_target("redis:alias"),
            Some("redis".to_string())
        );
    }

    #[test]
    fn link_target_with_underscore() {
        assert_eq!(
            parse_link_target("/redis_1:/app/redis"),
            Some("redis_1".to_string())
        );
    }

    #[test]
    fn link_target_empty_is_none() {
        assert_eq!(parse_link_target(""), None);
    }

    // ── topological_sort ──────────────────────────────────────────────────────

    fn make_container(name: &str) -> docker::ContainerInfo {
        docker::ContainerInfo {
            id: format!("{name}_id"),
            name: name.to_string(),
            image: format!("{name}:latest"),
            image_id: "sha256:abc".to_string(),
            state: docker::ContainerState::Running,
            labels: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn topo_sort_no_deps_preserves_all() {
        let containers = vec![make_container("a"), make_container("b"), make_container("c")];
        let dep_graph = HashMap::new();
        let result = topological_sort(&containers, &dep_graph);
        assert_eq!(result.len(), 3);
        let names: HashSet<&str> = result.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains("a") && names.contains("b") && names.contains("c"));
    }

    #[test]
    fn topo_sort_linear_chain_web_before_db() {
        // web depends on db → web is a leaf (no dependents), db is updated last
        let containers = vec![make_container("web"), make_container("db")];
        let mut dep_graph = HashMap::new();
        dep_graph.insert("web".to_string(), vec!["db".to_string()]);
        dep_graph.insert("db".to_string(), vec![]);
        let result = topological_sort(&containers, &dep_graph);
        assert_eq!(result.len(), 2);
        let web_pos = result.iter().position(|c| c.name == "web").unwrap();
        let db_pos = result.iter().position(|c| c.name == "db").unwrap();
        assert!(web_pos < db_pos, "web should come before db");
    }

    #[test]
    fn topo_sort_two_leaves_one_root() {
        // web → db, cache → db: both web and cache should precede db
        let containers = vec![
            make_container("web"),
            make_container("cache"),
            make_container("db"),
        ];
        let mut dep_graph = HashMap::new();
        dep_graph.insert("web".to_string(), vec!["db".to_string()]);
        dep_graph.insert("cache".to_string(), vec!["db".to_string()]);
        dep_graph.insert("db".to_string(), vec![]);
        let result = topological_sort(&containers, &dep_graph);
        assert_eq!(result.len(), 3);
        let db_pos = result.iter().position(|c| c.name == "db").unwrap();
        let web_pos = result.iter().position(|c| c.name == "web").unwrap();
        let cache_pos = result.iter().position(|c| c.name == "cache").unwrap();
        assert!(web_pos < db_pos);
        assert!(cache_pos < db_pos);
    }

    #[test]
    fn topo_sort_cycle_still_returns_all() {
        // A depends on B, B depends on A — cycle
        let containers = vec![make_container("a"), make_container("b")];
        let mut dep_graph = HashMap::new();
        dep_graph.insert("a".to_string(), vec!["b".to_string()]);
        dep_graph.insert("b".to_string(), vec!["a".to_string()]);
        let result = topological_sort(&containers, &dep_graph);
        assert_eq!(result.len(), 2);
    }

    // ── build_dependency_graph ────────────────────────────────────────────────

    #[test]
    fn dep_graph_depends_on_label() {
        let mut labels = std::collections::HashMap::new();
        labels.insert("saurron.depends-on".to_string(), "db".to_string());
        let web = docker::ContainerInfo {
            id: "web_id".to_string(),
            name: "web".to_string(),
            image: "web:latest".to_string(),
            image_id: "sha256:abc".to_string(),
            state: docker::ContainerState::Running,
            labels,
        };
        let db = make_container("db");
        let containers = vec![web, db];
        let graph = build_dependency_graph(&containers, &HashMap::new());
        assert!(graph["web"].contains(&"db".to_string()));
        assert!(graph["db"].is_empty());
    }

    #[test]
    fn dep_graph_unknown_dep_ignored() {
        let mut labels = std::collections::HashMap::new();
        labels.insert("saurron.depends-on".to_string(), "unknown_svc".to_string());
        let web = docker::ContainerInfo {
            id: "web_id".to_string(),
            name: "web".to_string(),
            image: "web:latest".to_string(),
            image_id: "sha256:abc".to_string(),
            state: docker::ContainerState::Running,
            labels,
        };
        let containers = vec![web];
        let graph = build_dependency_graph(&containers, &HashMap::new());
        assert!(graph["web"].is_empty());
    }

    #[test]
    fn dep_graph_network_mode_container() {
        let containers = vec![make_container("app"), make_container("db")];
        let mut inspect_map: HashMap<String, bollard::models::ContainerInspectResponse> =
            HashMap::new();
        inspect_map.insert(
            "app".to_string(),
            bollard::models::ContainerInspectResponse {
                host_config: Some(bollard::models::HostConfig {
                    network_mode: Some("container:db".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        let graph = build_dependency_graph(&containers, &inspect_map);
        assert!(graph["app"].contains(&"db".to_string()));
    }

    // ── resolve_bool_override ─────────────────────────────────────────────────

    #[test]
    fn override_global_wins_when_gtp() {
        assert!(resolve_bool_override(true, true, Some(false)));
    }

    #[test]
    fn override_label_wins_when_not_gtp() {
        assert!(resolve_bool_override(false, false, Some(true)));
    }

    #[test]
    fn override_falls_back_to_global_when_no_label() {
        assert!(resolve_bool_override(true, false, None));
    }

    // ── extract_run_config ────────────────────────────────────────────────────

    #[test]
    fn extract_run_config_all_none_gives_all_none() {
        let inspect = bollard::models::ContainerInspectResponse::default();
        let run_cfg = extract_run_config(&inspect);
        assert!(run_cfg.hostname.is_none());
        assert!(run_cfg.env.is_none());
        assert!(run_cfg.binds.is_none());
        assert!(run_cfg.networks.is_none());
    }

    #[test]
    fn extract_run_config_copies_env_and_labels() {
        let mut map = HashMap::new();
        map.insert("com.example.app".to_string(), "test".to_string());
        let inspect = bollard::models::ContainerInspectResponse {
            config: Some(bollard::models::ContainerConfig {
                env: Some(vec!["FOO=bar".to_string()]),
                labels: Some(map.clone()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        assert_eq!(run_cfg.env, Some(vec!["FOO=bar".to_string()]));
        assert_eq!(run_cfg.labels, Some(map));
    }
}
