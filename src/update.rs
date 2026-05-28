use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::Result;
use chrono::{DateTime, Utc};
use tracing::{debug, info, warn};

use crate::{audit, config, docker, registry, selfupdate};

// ── Duration parser ───────────────────────────────────────────────────────────

/// Parse a duration string of the form `<N><unit>` where unit is `s`, `m`, or `h`.
/// A bare integer is treated as seconds.
pub(crate) fn parse_duration_secs(s: &str) -> Result<u64> {
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
    pub exposed_ports: Option<Vec<String>>,
    pub healthcheck: Option<bollard::models::HealthConfig>,
    pub volumes: Option<Vec<String>>,
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
    pub group_add: Option<Vec<String>>,
    pub mounts: Option<Vec<bollard::models::Mount>>,
    pub security_opt: Option<Vec<String>>,
    pub memory: Option<i64>,
    pub memory_swap: Option<i64>,
    pub memory_reservation: Option<i64>,
    pub nano_cpus: Option<i64>,
    pub cpu_shares: Option<i64>,
    pub cpu_period: Option<i64>,
    pub cpu_quota: Option<i64>,
    pub cpuset_cpus: Option<String>,
    pub cpuset_mems: Option<String>,
    pub tmpfs: Option<HashMap<String, String>>,
    pub dns: Option<Vec<String>>,
    pub dns_search: Option<Vec<String>>,
    pub dns_options: Option<Vec<String>>,
    pub runtime: Option<String>,
    pub sysctls: Option<HashMap<String, String>>,
    pub pid_mode: Option<String>,
    pub ipc_mode: Option<String>,
    pub userns_mode: Option<String>,
    pub readonly_rootfs: Option<bool>,
    pub pids_limit: Option<i64>,
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
        healthcheck: cfg.and_then(|c| c.healthcheck.clone()),
        volumes: cfg.and_then(|c| c.volumes.clone()),
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
        group_add: hc.and_then(|h| h.group_add.clone()),
        mounts: hc.and_then(|h| h.mounts.clone()),
        security_opt: hc.and_then(|h| h.security_opt.clone()),
        memory: hc.and_then(|h| h.memory),
        memory_swap: hc.and_then(|h| h.memory_swap),
        memory_reservation: hc.and_then(|h| h.memory_reservation),
        nano_cpus: hc.and_then(|h| h.nano_cpus),
        cpu_shares: hc.and_then(|h| h.cpu_shares),
        cpu_period: hc.and_then(|h| h.cpu_period),
        cpu_quota: hc.and_then(|h| h.cpu_quota),
        cpuset_cpus: hc.and_then(|h| h.cpuset_cpus.clone()),
        cpuset_mems: hc.and_then(|h| h.cpuset_mems.clone()),
        tmpfs: hc.and_then(|h| h.tmpfs.clone()),
        dns: hc.and_then(|h| h.dns.clone()),
        dns_search: hc.and_then(|h| h.dns_search.clone()),
        dns_options: hc.and_then(|h| h.dns_options.clone()),
        runtime: hc.and_then(|h| h.runtime.clone()),
        sysctls: hc.and_then(|h| h.sysctls.clone()),
        pid_mode: hc.and_then(|h| h.pid_mode.clone()),
        ipc_mode: hc.and_then(|h| h.ipc_mode.clone()),
        userns_mode: hc.and_then(|h| h.userns_mode.clone()),
        readonly_rootfs: hc.and_then(|h| h.readonly_rootfs),
        pids_limit: hc.and_then(|h| h.pids_limit),
        networks: ns.and_then(|n| n.networks.clone()),
    }
}

fn build_create_config(
    run_cfg: &ContainerRunConfig,
    new_image: &str,
    stop_signal_override: Option<&str>,
) -> bollard::models::ContainerCreateBody {
    let networking_config =
        run_cfg
            .networks
            .as_ref()
            .map(|nets| bollard::models::NetworkingConfig {
                endpoints_config: Some(nets.clone()),
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
        group_add: run_cfg.group_add.clone(),
        mounts: run_cfg.mounts.clone(),
        security_opt: run_cfg.security_opt.clone(),
        memory: run_cfg.memory,
        memory_swap: run_cfg.memory_swap,
        memory_reservation: run_cfg.memory_reservation,
        nano_cpus: run_cfg.nano_cpus,
        cpu_shares: run_cfg.cpu_shares,
        cpu_period: run_cfg.cpu_period,
        cpu_quota: run_cfg.cpu_quota,
        cpuset_cpus: run_cfg.cpuset_cpus.clone(),
        cpuset_mems: run_cfg.cpuset_mems.clone(),
        tmpfs: run_cfg.tmpfs.clone(),
        dns: run_cfg.dns.clone(),
        dns_search: run_cfg.dns_search.clone(),
        dns_options: run_cfg.dns_options.clone(),
        runtime: run_cfg.runtime.clone(),
        sysctls: run_cfg.sysctls.clone(),
        pid_mode: run_cfg.pid_mode.clone(),
        ipc_mode: run_cfg.ipc_mode.clone(),
        userns_mode: run_cfg.userns_mode.clone(),
        readonly_rootfs: run_cfg.readonly_rootfs,
        pids_limit: run_cfg.pids_limit,
        ..Default::default()
    });

    let effective_stop_signal = stop_signal_override
        .map(|s| s.to_string())
        .or_else(|| run_cfg.stop_signal.clone());

    bollard::models::ContainerCreateBody {
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
        healthcheck: run_cfg.healthcheck.clone(),
        volumes: run_cfg.volumes.clone(),
        image: Some(new_image.to_string()),
        host_config,
        networking_config,
        ..Default::default()
    }
}

// ── Docker error helpers ──────────────────────────────────────────────────────

/// If `e` wraps a `bollard::errors::Error::DockerResponseServerError`, return
/// the HTTP status code and daemon message as separate values so they can be
/// logged as individual structured fields.  Returns `None` for all other error
/// types, including non-Docker bollard errors.
fn extract_docker_server_error(e: &anyhow::Error) -> Option<(u16, String)> {
    use bollard::errors::Error as DockerError;
    match e.downcast_ref::<DockerError>() {
        Some(DockerError::DockerResponseServerError {
            status_code,
            message,
        }) => Some((*status_code, message.clone())),
        _ => None,
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

        if let Some(inspect) = inspect_map.get(&c.name)
            && let Some(hc) = &inspect.host_config
        {
            // 2. Docker --link
            for link in hc.links.iter().flatten() {
                if let Some(target) = parse_link_target(link)
                    && name_set.contains(target.as_str())
                {
                    deps.push(target);
                }
            }
            // 3. network_mode: container:<name>
            if let Some(nm) = &hc.network_mode
                && let Some(dep_name) = nm.strip_prefix("container:")
                && name_set.contains(dep_name)
            {
                deps.push(dep_name.to_string());
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
    RolledBack {
        old_image: String,
        old_digest: String,
        attempted_image: String,
        attempted_digest: String,
        reason: String,
    },
    Failed(anyhow::Error),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerOutcome {
    Updated,
    RolledBack,
    Failed,
    Skipped,
    UpToDate,
}

#[derive(Debug, serde::Serialize)]
pub struct ContainerReport {
    pub name: String,
    pub outcome: ContainerOutcome,
    pub old_image: Option<String>,
    pub new_image: Option<String>,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct SessionReport {
    pub containers: Vec<ContainerReport>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

impl SessionReport {
    pub fn record(&mut self, name: &str, result: &UpdateResult, old_image: Option<String>) {
        let (outcome, old_img, new_img) = match result {
            UpdateResult::Updated { old_image, new_image, .. } => (
                ContainerOutcome::Updated,
                Some(old_image.clone()),
                Some(new_image.clone()),
            ),
            UpdateResult::RolledBack { old_image, attempted_image, .. } => (
                ContainerOutcome::RolledBack,
                Some(old_image.clone()),
                Some(attempted_image.clone()),
            ),
            UpdateResult::Skipped(_) => (ContainerOutcome::Skipped, old_image, None),
            UpdateResult::Failed(_) => (ContainerOutcome::Failed, old_image, None),
            UpdateResult::UpToDate => (ContainerOutcome::UpToDate, None, None),
        };
        self.containers.push(ContainerReport {
            name: name.to_string(),
            outcome,
            old_image: old_img,
            new_image: new_img,
        });
    }
}

// ── Rollback / startup monitoring ────────────────────────────────────────────

#[derive(Debug, PartialEq)]
enum RollbackTrigger {
    NonZeroExit(i64),
    HealthcheckFailure,
    StartupTimeout,
}

impl RollbackTrigger {
    fn reason_str(&self) -> String {
        match self {
            RollbackTrigger::NonZeroExit(code) => format!("exit_code={code}"),
            RollbackTrigger::HealthcheckFailure => "healthcheck_failed".to_string(),
            RollbackTrigger::StartupTimeout => "startup_timeout".to_string(),
        }
    }
}

#[derive(Debug, PartialEq)]
enum StartupEval {
    Ok,
    Rollback(RollbackTrigger),
    Continue,
}

/// Pure per-poll decision: given a container state snapshot, decide whether
/// startup succeeded, failed (rollback), or is still in progress (continue).
fn evaluate_startup_state(
    state: &bollard::models::ContainerState,
    on_exit_code: bool,
    on_healthcheck: bool,
) -> StartupEval {
    use bollard::models::{ContainerStateStatusEnum, HealthStatusEnum};

    let running = state.running.unwrap_or(false);
    let exited = state.status == Some(ContainerStateStatusEnum::EXITED);

    // Non-zero exit check
    if on_exit_code && !running && exited {
        let code = state.exit_code.unwrap_or(0);
        if code != 0 {
            return StartupEval::Rollback(RollbackTrigger::NonZeroExit(code));
        }
    }

    // Healthcheck check (only meaningful when container is running)
    if running {
        if let Some(health) = &state.health {
            match health.status {
                Some(HealthStatusEnum::UNHEALTHY) if on_healthcheck => {
                    return StartupEval::Rollback(RollbackTrigger::HealthcheckFailure);
                }
                Some(HealthStatusEnum::STARTING) | Some(HealthStatusEnum::EMPTY) => {
                    // Still initializing
                    return StartupEval::Continue;
                }
                _ => {
                    // HEALTHY, NONE, or UNHEALTHY with on_healthcheck=false
                    return StartupEval::Ok;
                }
            }
        }
        // Running with no healthcheck configured → success
        return StartupEval::Ok;
    }

    StartupEval::Continue
}

async fn monitor_startup(
    docker: &docker::DockerClient,
    container_name: &str,
    new_id: &str,
    timeout_secs: u64,
    on_exit_code: bool,
    on_healthcheck: bool,
    on_timeout: bool,
) -> Result<(), RollbackTrigger> {
    use tokio::time::{Duration, Instant, sleep};

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        match docker.inspect_container(new_id).await {
            Ok(resp) => {
                if let Some(state) = &resp.state {
                    match evaluate_startup_state(state, on_exit_code, on_healthcheck) {
                        StartupEval::Ok => return Ok(()),
                        StartupEval::Rollback(trigger) => return Err(trigger),
                        StartupEval::Continue => {}
                    }
                }
            }
            Err(e) => {
                // Transient API error — log and keep polling
                tracing::debug!(
                    container = %container_name,
                    error = %e,
                    "transient inspect error during startup monitoring"
                );
            }
        }

        if Instant::now() >= deadline {
            if on_timeout {
                return Err(RollbackTrigger::StartupTimeout);
            } else {
                return Ok(());
            }
        }

        sleep(Duration::from_secs(1)).await;
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

/// True when the running container was started from an older version of the
/// local image — the tag was re-pulled without the container being restarted.
fn local_image_is_newer(running_image_id: &str, local_image_id: &str) -> bool {
    !running_image_id.is_empty() && !local_image_id.is_empty() && running_image_id != local_image_id
}

pub struct UpdateEngine<'a> {
    docker: &'a docker::DockerClient,
    registry: &'a registry::RegistryClient,
    config: &'a config::Config,
}

impl<'a> UpdateEngine<'a> {
    pub fn new(
        docker: &'a docker::DockerClient,
        registry: &'a registry::RegistryClient,
        config: &'a config::Config,
    ) -> Self {
        Self {
            docker,
            registry,
            config,
        }
    }

    pub async fn run_cycle(&self, containers: &[docker::ContainerInfo]) -> SessionReport {
        let started_at = Utc::now();
        let mut report = SessionReport { started_at, ..Default::default() };
        let own_id = selfupdate::detect_own_container_id();
        match &own_id {
            Some(id) => debug!(own_container_id = %id, "detected own container ID"),
            None => debug!(
                "could not detect own container ID; self-update will use regular update path"
            ),
        }

        // Phase A: scan all containers for staleness
        let mut stale: Vec<(docker::ContainerInfo, registry::StaleInfo)> = Vec::new();
        for container in containers {
            match self.check_freshness(container).await {
                registry::FreshnessResult::UpToDate => {
                    debug!(container = %container.name, "image up to date");
                    report.record(&container.name, &UpdateResult::UpToDate, None);
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
                    report.record(&container.name, &UpdateResult::Skipped(reason), Some(container.image.clone()));
                }
                registry::FreshnessResult::Error(reason) => {
                    warn!(container = %container.name, reason, "freshness check failed");
                    report.record(&container.name, &UpdateResult::Skipped(reason), Some(container.image.clone()));
                }
            }
        }

        if stale.is_empty() {
            info!(total = containers.len(), "All containers up to date");
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
                    report.record(&c.name, &UpdateResult::Failed(e), Some(c.image.clone()));
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

        // Phase D: update each stale container in dependency order.
        // Self-container (if detected) is deferred to after all others.
        let mut self_update_queue: Vec<&docker::ContainerInfo> = Vec::new();

        for container in &ordered {
            let Some(stale_info) = stale_map.get(&container.name) else {
                continue;
            };
            let Some(inspect) = inspect_map.get(&container.name) else {
                continue;
            };

            // Defer self-container to the end so other containers update first
            if own_id
                .as_deref()
                .is_some_and(|oid| selfupdate::is_self_container(&container.id, oid))
            {
                info!(container = %container.name, "deferring self-update to end of cycle");
                self_update_queue.push(container);
                continue;
            }

            let result = self.update_one(container, stale_info, inspect).await;
            match &result {
                UpdateResult::Failed(e) => {
                    warn!(container = %container.name, error = %e, "update failed");
                }
                UpdateResult::RolledBack { reason, .. } => {
                    warn!(container = %container.name, reason, "update rolled back");
                }
                _ => {}
            }
            report.record(&container.name, &result, Some(container.image.clone()));
        }

        // Phase D2: self-update (runs last)
        for container in &self_update_queue {
            let Some(stale_info) = stale_map.get(&container.name) else {
                continue;
            };
            let Some(inspect) = inspect_map.get(&container.name) else {
                continue;
            };
            info!(container = %container.name, "beginning self-update");
            let result = self.self_update_one(container, stale_info, inspect).await;
            if let UpdateResult::Failed(ref e) = result {
                warn!(container = %container.name, error = %e, "self-update failed");
            }
            report.record(&container.name, &result, Some(container.image.clone()));
        }

        // Phase E: session summary
        let updated = report.containers.iter().filter(|c| c.outcome == ContainerOutcome::Updated).count();
        let rolled_back = report.containers.iter().filter(|c| c.outcome == ContainerOutcome::RolledBack).count();
        let skipped = report.containers.iter().filter(|c| c.outcome == ContainerOutcome::Skipped).count();
        let failed = report.containers.iter().filter(|c| c.outcome == ContainerOutcome::Failed).count();
        let up_to_date = report.containers.iter().filter(|c| c.outcome == ContainerOutcome::UpToDate).count();
        info!(updated, rolled_back, skipped, failed, up_to_date, "Update cycle complete");

        report.completed_at = Utc::now();
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

        // Only substitute the first RepoTag when the container was started from a
        // bare digest — in that case container.image is "sha256:..." with no tag to
        // query. For all normal tagged references, use container.image as-is so that
        // the registry query targets the correct repo/tag, not a shared base image tag.
        let image_for_check = if container.image.starts_with("sha256:") {
            image_info.name.as_deref().unwrap_or(&container.image)
        } else {
            &container.image
        };
        let labels = container.saurron_labels();
        let allow_pre = labels.semver_pre_release.unwrap_or(false);
        let strategy = labels
            .non_semver_strategy
            .as_deref()
            .map(registry::parse_non_semver_strategy)
            .unwrap_or_default();

        let freshness = self
            .registry
            .check_freshness(
                image_for_check,
                image_info.digest.as_deref(),
                allow_pre,
                strategy,
            )
            .await;

        // The registry comparison above uses the local image's manifest digest, not
        // the running container's image ID. If the local image tag was re-pulled
        // (e.g. by another tool, or left behind after a rollback) without restarting
        // the container, the local digest matches the registry but the container is
        // still running the old image. Detect this by comparing image IDs.
        if matches!(freshness, registry::FreshnessResult::UpToDate)
            && let Some(ref local_id) = image_info.id
            && local_image_is_newer(&container.image_id, local_id)
        {
            info!(
                container = %container.name,
                running_image_id = %container.image_id,
                local_image_id = %local_id,
                "container is running an outdated local image; treating as stale"
            );
            return registry::FreshnessResult::Stale(registry::StaleInfo {
                current_digest: container.image_id.clone(),
                new_image: image_for_check.to_string(),
                new_digest: image_info.digest.clone().unwrap_or_default(),
            });
        }

        freshness
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
        let effective_monitor_only = resolve_bool_override(
            cfg.monitor_only,
            cfg.global_takes_precedence,
            labels.monitor_only,
        );
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
                .pull_image(
                    &stale_info.new_image,
                    self.registry.credentials_for_image(&stale_info.new_image),
                )
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

        // Step 8: startup monitoring + rollback
        let startup_timeout = parse_duration_secs(&cfg.rollback.startup_timeout).unwrap_or(30);
        match monitor_startup(
            self.docker,
            &container.name,
            &new_id,
            startup_timeout,
            cfg.rollback.on_exit_code,
            cfg.rollback.on_healthcheck,
            cfg.rollback.on_timeout,
        )
        .await
        {
            Ok(()) => {
                info!(container = %container.name, new_id = %new_id, "container started successfully");
            }
            Err(trigger) => {
                let reason = trigger.reason_str();
                warn!(
                    container = %container.name,
                    new_id = %new_id,
                    reason,
                    "startup check failed — rolling back"
                );

                // Stop and remove failed new container
                let _ = self.docker.stop_container(&new_id, 10).await;
                let _ = self.docker.remove_container(&new_id).await;

                // Recreate old container from original run config + old image
                let rollback_cfg =
                    build_create_config(&run_cfg, &old_image, effective_stop_signal.as_deref());
                match self
                    .docker
                    .create_container(&container.name, rollback_cfg)
                    .await
                {
                    Err(e) => {
                        return UpdateResult::Failed(e.context(format!(
                            "rollback failed: could not recreate '{}' with old image",
                            container.name
                        )));
                    }
                    Ok(restored_id) => {
                        if let Err(e) = self.docker.start_container(&restored_id).await {
                            return UpdateResult::Failed(e.context(format!(
                                "rollback failed: could not start restored container '{}'",
                                container.name
                            )));
                        }
                        audit::audit_rollback(
                            &container.name,
                            &restored_id,
                            &stale_info.new_image,
                            &new_digest,
                            &old_image,
                            &old_digest,
                            &reason,
                        );
                        return UpdateResult::RolledBack {
                            old_image,
                            old_digest,
                            attempted_image: stale_info.new_image.clone(),
                            attempted_digest: new_digest,
                            reason,
                        };
                    }
                }
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
                if let Some((http_status, docker_message)) = extract_docker_server_error(&e) {
                    warn!(
                        container = %container.name,
                        image = %old_image,
                        http_status,
                        docker_message,
                        "old image removal failed (non-fatal)"
                    );
                } else {
                    warn!(
                        container = %container.name,
                        image = %old_image,
                        error = %format!("{e:#}"),
                        "old image removal failed (non-fatal)"
                    );
                }
            }
        }

        UpdateResult::Updated {
            old_image,
            old_digest,
            new_image: stale_info.new_image.clone(),
            new_digest,
        }
    }

    /// Self-update path: rename own container to a temp name, start replacement
    /// under the original name, monitor it. On failure, rename self back.
    async fn self_update_one(
        &self,
        container: &docker::ContainerInfo,
        stale_info: &registry::StaleInfo,
        inspect: &bollard::models::ContainerInspectResponse,
    ) -> UpdateResult {
        let labels = container.saurron_labels();
        let cfg = self.config;

        let effective_monitor_only = resolve_bool_override(
            cfg.monitor_only,
            cfg.global_takes_precedence,
            labels.monitor_only,
        );
        let effective_no_pull =
            resolve_bool_override(cfg.no_pull, cfg.global_takes_precedence, labels.no_pull);
        let effective_stop_signal: Option<String> = labels.stop_signal.clone();

        if effective_monitor_only {
            info!(
                container = %container.name,
                new_image = %stale_info.new_image,
                "monitor-only: skipping self-update"
            );
            return UpdateResult::Skipped("monitor-only".to_string());
        }

        let old_image = container.image.clone();
        let old_digest = stale_info.current_digest.clone();

        // Step 1: pull new image
        if !effective_no_pull {
            info!(container = %container.name, image = %stale_info.new_image, "pulling new image for self-update");
            if let Err(e) = self
                .docker
                .pull_image(
                    &stale_info.new_image,
                    self.registry.credentials_for_image(&stale_info.new_image),
                )
                .await
            {
                return UpdateResult::Failed(e.context(format!(
                    "self-update pull failed for '{}'",
                    stale_info.new_image
                )));
            }
        }

        // Step 2: get new digest
        let new_digest = match self
            .docker
            .get_local_image_info(&stale_info.new_image)
            .await
        {
            Ok(info) => info.digest.unwrap_or_else(|| stale_info.new_digest.clone()),
            Err(_) => stale_info.new_digest.clone(),
        };

        // Step 3: extract run config
        let run_cfg = extract_run_config(inspect);

        // Step 4: rename self to temp name (freeing our original name)
        let temp_name = selfupdate::temp_container_name(&container.name);
        info!(
            container = %container.name,
            temp_name = %temp_name,
            "renaming self for self-update"
        );
        if let Err(e) = self
            .docker
            .rename_container(&container.id, &temp_name)
            .await
        {
            return UpdateResult::Failed(e.context(format!(
                "self-update rename failed for '{}'",
                container.name
            )));
        }

        // Step 5: create new container under original name
        let create_cfg = build_create_config(
            &run_cfg,
            &stale_info.new_image,
            effective_stop_signal.as_deref(),
        );
        info!(
            container = %container.name,
            new_image = %stale_info.new_image,
            "creating self-update replacement container"
        );
        let new_id = match self
            .docker
            .create_container(&container.name, create_cfg)
            .await
        {
            Ok(id) => id,
            Err(e) => {
                // Rename self back on failure
                let _ = self
                    .docker
                    .rename_container(&container.id, &container.name)
                    .await;
                return UpdateResult::Failed(e.context(format!(
                    "self-update create failed for '{}'",
                    container.name
                )));
            }
        };

        // Step 6: start new container
        if let Err(e) = self.docker.start_container(&new_id).await {
            let _ = self.docker.remove_container(&new_id).await;
            let _ = self
                .docker
                .rename_container(&container.id, &container.name)
                .await;
            return UpdateResult::Failed(
                e.context(format!("self-update start failed for '{}'", container.name)),
            );
        }

        // Step 7: monitor startup
        let startup_timeout = parse_duration_secs(&cfg.rollback.startup_timeout).unwrap_or(30);
        match monitor_startup(
            self.docker,
            &container.name,
            &new_id,
            startup_timeout,
            cfg.rollback.on_exit_code,
            cfg.rollback.on_healthcheck,
            cfg.rollback.on_timeout,
        )
        .await
        {
            Ok(()) => {
                info!(
                    container = %container.name,
                    new_id = %new_id,
                    "self-update replacement started successfully; stopping old container"
                );
                // Spawn the stop so this task can proceed to audit before the
                // current process receives SIGTERM. A blocking await would
                // deadlock: Docker stops us, we never return, audit is lost.
                let docker = self.docker.clone();
                let old_id = container.id.clone();
                let container_name = container.name.clone();
                tokio::spawn(async move {
                    if let Err(e) = docker.stop_container(&old_id, 10).await {
                        warn!(
                            container = %container_name,
                            error = %e,
                            "self-update: failed to stop old container"
                        );
                    }
                });
            }
            Err(trigger) => {
                let reason = trigger.reason_str();
                warn!(
                    container = %container.name,
                    new_id = %new_id,
                    reason,
                    "self-update replacement failed startup — restoring old container"
                );
                // Stop and remove failed replacement
                let _ = self.docker.stop_container(&new_id, 10).await;
                let _ = self.docker.remove_container(&new_id).await;
                // Rename self back to original name
                if let Err(e) = self
                    .docker
                    .rename_container(&container.id, &container.name)
                    .await
                {
                    return UpdateResult::Failed(e.context(format!(
                        "self-update recovery rename failed for '{}': could not restore original name",
                        container.name
                    )));
                }
                return UpdateResult::Failed(anyhow::anyhow!(
                    "self-update failed ({}); old container restored as '{}'",
                    reason,
                    container.name
                ));
            }
        }

        // Step 8: audit
        audit::audit_update(
            &container.name,
            &new_id,
            &old_image,
            &old_digest,
            &stale_info.new_image,
            &new_digest,
        );

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
        assert_eq!(parse_link_target("redis:alias"), Some("redis".to_string()));
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
        let containers = vec![
            make_container("a"),
            make_container("b"),
            make_container("c"),
        ];
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

    // ── build_create_config ───────────────────────────────────────────────────

    fn default_run_cfg() -> ContainerRunConfig {
        ContainerRunConfig {
            hostname: None,
            domainname: None,
            user: None,
            env: None,
            cmd: None,
            entrypoint: None,
            working_dir: None,
            tty: None,
            open_stdin: None,
            stop_signal: None,
            labels: None,
            exposed_ports: None,
            binds: None,
            volumes_from: None,
            port_bindings: None,
            restart_policy: None,
            network_mode: None,
            links: None,
            extra_hosts: None,
            cap_add: None,
            cap_drop: None,
            privileged: None,
            devices: None,
            log_config: None,
            shm_size: None,
            ulimits: None,
            init: None,
            group_add: None,
            mounts: None,
            security_opt: None,
            memory: None,
            memory_swap: None,
            memory_reservation: None,
            nano_cpus: None,
            cpu_shares: None,
            cpu_period: None,
            cpu_quota: None,
            cpuset_cpus: None,
            cpuset_mems: None,
            tmpfs: None,
            dns: None,
            dns_search: None,
            dns_options: None,
            runtime: None,
            sysctls: None,
            pid_mode: None,
            ipc_mode: None,
            userns_mode: None,
            readonly_rootfs: None,
            pids_limit: None,
            healthcheck: None,
            volumes: None,
            networks: None,
        }
    }

    #[test]
    fn build_create_config_sets_new_image() {
        let run_cfg = default_run_cfg();
        let cfg = build_create_config(&run_cfg, "myrepo/myapp:2.0.0", None);
        assert_eq!(cfg.image, Some("myrepo/myapp:2.0.0".to_string()));
    }

    #[test]
    fn build_create_config_stop_signal_override_takes_precedence() {
        let mut run_cfg = default_run_cfg();
        run_cfg.stop_signal = Some("SIGKILL".to_string());
        let cfg = build_create_config(&run_cfg, "img:latest", Some("SIGTERM"));
        assert_eq!(cfg.stop_signal, Some("SIGTERM".to_string()));
    }

    #[test]
    fn build_create_config_stop_signal_from_run_config_when_no_override() {
        let mut run_cfg = default_run_cfg();
        run_cfg.stop_signal = Some("SIGKILL".to_string());
        let cfg = build_create_config(&run_cfg, "img:latest", None);
        assert_eq!(cfg.stop_signal, Some("SIGKILL".to_string()));
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

    #[test]
    fn extract_run_config_with_env() {
        let inspect = bollard::models::ContainerInspectResponse {
            config: Some(bollard::models::ContainerConfig {
                env: Some(vec!["PATH=/usr/bin".to_string(), "HOME=/root".to_string()]),
                hostname: Some("myhost".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        assert_eq!(
            run_cfg.env,
            Some(vec!["PATH=/usr/bin".to_string(), "HOME=/root".to_string()])
        );
        assert_eq!(run_cfg.hostname, Some("myhost".to_string()));
    }

    // ── evaluate_startup_state ────────────────────────────────────────────────

    fn make_state(
        running: bool,
        status: Option<&str>,
        exit_code: Option<i64>,
        health_status: Option<bollard::models::HealthStatusEnum>,
    ) -> bollard::models::ContainerState {
        use bollard::models::{ContainerStateStatusEnum, Health};
        let parsed_status = status.map(|s| match s {
            "running" => ContainerStateStatusEnum::RUNNING,
            "exited" => ContainerStateStatusEnum::EXITED,
            "created" => ContainerStateStatusEnum::CREATED,
            _ => ContainerStateStatusEnum::EMPTY,
        });
        let health = health_status.map(|hs| Health {
            status: Some(hs),
            ..Default::default()
        });
        bollard::models::ContainerState {
            running: Some(running),
            status: parsed_status,
            exit_code,
            health,
            ..Default::default()
        }
    }

    #[test]
    fn eval_running_no_healthcheck_is_ok() {
        let state = make_state(true, Some("running"), None, None);
        assert_eq!(evaluate_startup_state(&state, true, true), StartupEval::Ok);
    }

    #[test]
    fn eval_running_healthy_is_ok() {
        let state = make_state(
            true,
            Some("running"),
            None,
            Some(bollard::models::HealthStatusEnum::HEALTHY),
        );
        assert_eq!(evaluate_startup_state(&state, true, true), StartupEval::Ok);
    }

    #[test]
    fn eval_running_health_none_is_ok() {
        let state = make_state(
            true,
            Some("running"),
            None,
            Some(bollard::models::HealthStatusEnum::NONE),
        );
        assert_eq!(evaluate_startup_state(&state, true, true), StartupEval::Ok);
    }

    #[test]
    fn eval_running_health_starting_is_continue() {
        let state = make_state(
            true,
            Some("running"),
            None,
            Some(bollard::models::HealthStatusEnum::STARTING),
        );
        assert_eq!(
            evaluate_startup_state(&state, true, true),
            StartupEval::Continue
        );
    }

    #[test]
    fn eval_running_unhealthy_with_on_healthcheck_is_rollback() {
        let state = make_state(
            true,
            Some("running"),
            None,
            Some(bollard::models::HealthStatusEnum::UNHEALTHY),
        );
        assert_eq!(
            evaluate_startup_state(&state, true, true),
            StartupEval::Rollback(RollbackTrigger::HealthcheckFailure)
        );
    }

    #[test]
    fn eval_running_unhealthy_without_on_healthcheck_is_continue() {
        let state = make_state(
            true,
            Some("running"),
            None,
            Some(bollard::models::HealthStatusEnum::UNHEALTHY),
        );
        // on_healthcheck=false: unhealthy is ignored, but container is running → Ok
        // (health check NONE/HEALTHY path not taken; UNHEALTHY with on_healthcheck=false falls through to running=true → Ok)
        assert_eq!(evaluate_startup_state(&state, true, false), StartupEval::Ok);
    }

    #[test]
    fn eval_exited_nonzero_with_on_exit_code_is_rollback() {
        let state = make_state(false, Some("exited"), Some(1), None);
        assert_eq!(
            evaluate_startup_state(&state, true, true),
            StartupEval::Rollback(RollbackTrigger::NonZeroExit(1))
        );
    }

    #[test]
    fn eval_exited_nonzero_without_on_exit_code_is_continue() {
        let state = make_state(false, Some("exited"), Some(1), None);
        assert_eq!(
            evaluate_startup_state(&state, false, true),
            StartupEval::Continue
        );
    }

    #[test]
    fn eval_exited_zero_is_continue() {
        let state = make_state(false, Some("exited"), Some(0), None);
        assert_eq!(
            evaluate_startup_state(&state, true, true),
            StartupEval::Continue
        );
    }

    // ── RollbackTrigger::reason_str ───────────────────────────────────────────

    #[test]
    fn trigger_reason_non_zero_exit() {
        assert_eq!(
            RollbackTrigger::NonZeroExit(137).reason_str(),
            "exit_code=137"
        );
    }

    #[test]
    fn trigger_reason_healthcheck() {
        assert_eq!(
            RollbackTrigger::HealthcheckFailure.reason_str(),
            "healthcheck_failed"
        );
    }

    #[test]
    fn trigger_reason_timeout() {
        assert_eq!(
            RollbackTrigger::StartupTimeout.reason_str(),
            "startup_timeout"
        );
    }

    // ── topological_sort — diamond dependency ─────────────────────────────────

    #[test]
    fn topological_sort_diamond_deps() {
        // A depends on B and C; B and C both depend on D.
        // Update order (dependents first): A, then B/C, then D.
        let a = make_container("a");
        let b = make_container("b");
        let c = make_container("c");
        let d = make_container("d");
        let containers = vec![a, b, c, d];
        let mut deps: HashMap<String, Vec<String>> = HashMap::new();
        deps.insert("a".to_string(), vec!["b".to_string(), "c".to_string()]);
        deps.insert("b".to_string(), vec!["d".to_string()]);
        deps.insert("c".to_string(), vec!["d".to_string()]);
        deps.insert("d".to_string(), vec![]);
        let sorted = topological_sort(&containers, &deps);
        let names: Vec<&str> = sorted.iter().map(|c| c.name.as_str()).collect();
        let pos = |n: &str| names.iter().position(|&x| x == n).unwrap();
        assert!(pos("a") < pos("b"));
        assert!(pos("a") < pos("c"));
        assert!(pos("b") < pos("d"));
        assert!(pos("c") < pos("d"));
    }

    // ── extract_run_config — host_config fields ───────────────────────────────

    #[test]
    fn extract_run_config_copies_host_config_fields() {
        let inspect = bollard::models::ContainerInspectResponse {
            host_config: Some(bollard::models::HostConfig {
                binds: Some(vec!["/data:/data:ro".to_string()]),
                network_mode: Some("bridge".to_string()),
                privileged: Some(true),
                cap_add: Some(vec!["NET_ADMIN".to_string()]),
                cap_drop: Some(vec!["ALL".to_string()]),
                extra_hosts: Some(vec!["host.docker.internal:host-gateway".to_string()]),
                shm_size: Some(67_108_864),
                init: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        assert_eq!(run_cfg.binds, Some(vec!["/data:/data:ro".to_string()]));
        assert_eq!(run_cfg.network_mode, Some("bridge".to_string()));
        assert_eq!(run_cfg.privileged, Some(true));
        assert_eq!(run_cfg.cap_add, Some(vec!["NET_ADMIN".to_string()]));
        assert_eq!(run_cfg.cap_drop, Some(vec!["ALL".to_string()]));
        assert_eq!(
            run_cfg.extra_hosts,
            Some(vec!["host.docker.internal:host-gateway".to_string()])
        );
        assert_eq!(run_cfg.shm_size, Some(67_108_864));
        assert_eq!(run_cfg.init, Some(true));
    }

    #[test]
    fn extract_run_config_copies_volumes_from_and_links() {
        let inspect = bollard::models::ContainerInspectResponse {
            host_config: Some(bollard::models::HostConfig {
                volumes_from: Some(vec!["data-container".to_string()]),
                links: Some(vec!["/redis:/app/redis".to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        assert_eq!(
            run_cfg.volumes_from,
            Some(vec!["data-container".to_string()])
        );
        assert!(run_cfg.links.is_some());
    }

    // ── extract_run_config — network_settings ─────────────────────────────────

    #[test]
    fn extract_run_config_copies_network_settings() {
        let mut networks = HashMap::new();
        networks.insert(
            "mynet".to_string(),
            bollard::models::EndpointSettings::default(),
        );
        let inspect = bollard::models::ContainerInspectResponse {
            network_settings: Some(bollard::models::NetworkSettings {
                networks: Some(networks),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        assert!(run_cfg.networks.as_ref().unwrap().contains_key("mynet"));
    }

    // ── build_create_config — networking branch ───────────────────────────────

    #[test]
    fn build_create_config_with_networking_config() {
        let mut run_cfg = default_run_cfg();
        let mut networks = HashMap::new();
        networks.insert(
            "mynet".to_string(),
            bollard::models::EndpointSettings::default(),
        );
        run_cfg.networks = Some(networks);
        let cfg = build_create_config(&run_cfg, "img:latest", None);
        let nc = cfg.networking_config.unwrap();
        assert!(nc.endpoints_config.unwrap().contains_key("mynet"));
    }

    #[test]
    fn build_create_config_copies_host_config_fields() {
        let mut run_cfg = default_run_cfg();
        run_cfg.binds = Some(vec!["/data:/data:ro".to_string()]);
        run_cfg.cap_add = Some(vec!["NET_ADMIN".to_string()]);
        run_cfg.privileged = Some(true);
        run_cfg.shm_size = Some(67_108_864);
        run_cfg.init = Some(true);
        let cfg = build_create_config(&run_cfg, "img:latest", None);
        let hc = cfg.host_config.unwrap();
        assert_eq!(hc.binds, Some(vec!["/data:/data:ro".to_string()]));
        assert_eq!(hc.cap_add, Some(vec!["NET_ADMIN".to_string()]));
        assert_eq!(hc.privileged, Some(true));
        assert_eq!(hc.shm_size, Some(67_108_864));
        assert_eq!(hc.init, Some(true));
    }

    // ── extract_run_config / build_create_config — high-priority bug fixes ──────

    #[test]
    fn extract_run_config_copies_group_add() {
        let inspect = bollard::models::ContainerInspectResponse {
            host_config: Some(bollard::models::HostConfig {
                group_add: Some(vec!["999".to_string(), "docker".to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        assert_eq!(
            run_cfg.group_add,
            Some(vec!["999".to_string(), "docker".to_string()])
        );
    }

    #[test]
    fn extract_run_config_copies_healthcheck() {
        let inspect = bollard::models::ContainerInspectResponse {
            config: Some(bollard::models::ContainerConfig {
                healthcheck: Some(bollard::models::HealthConfig {
                    test: Some(vec![
                        "CMD".to_string(),
                        "curl".to_string(),
                        "-f".to_string(),
                        "http://localhost/health".to_string(),
                    ]),
                    interval: Some(30_000_000_000),
                    timeout: Some(10_000_000_000),
                    retries: Some(3),
                    start_period: Some(5_000_000_000),
                    start_interval: None,
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        let hc = run_cfg.healthcheck.expect("healthcheck should be Some");
        assert_eq!(
            hc.test,
            Some(vec![
                "CMD".to_string(),
                "curl".to_string(),
                "-f".to_string(),
                "http://localhost/health".to_string(),
            ])
        );
        assert_eq!(hc.retries, Some(3));
    }

    #[test]
    fn extract_run_config_copies_mounts() {
        let inspect = bollard::models::ContainerInspectResponse {
            host_config: Some(bollard::models::HostConfig {
                mounts: Some(vec![bollard::models::Mount {
                    target: Some("/data".to_string()),
                    source: Some("myvolume".to_string()),
                    typ: Some(bollard::models::MountTypeEnum::VOLUME),
                    read_only: Some(false),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        let mounts = run_cfg.mounts.expect("mounts should be Some");
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].target, Some("/data".to_string()));
        assert_eq!(mounts[0].source, Some("myvolume".to_string()));
    }

    #[test]
    fn extract_run_config_copies_volumes() {
        let inspect = bollard::models::ContainerInspectResponse {
            config: Some(bollard::models::ContainerConfig {
                volumes: Some(vec!["/data".to_string(), "/cache".to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        assert_eq!(
            run_cfg.volumes,
            Some(vec!["/data".to_string(), "/cache".to_string()])
        );
    }

    #[test]
    fn build_create_config_copies_group_add() {
        let mut run_cfg = default_run_cfg();
        run_cfg.group_add = Some(vec!["999".to_string()]);
        let cfg = build_create_config(&run_cfg, "img:latest", None);
        assert_eq!(
            cfg.host_config.unwrap().group_add,
            Some(vec!["999".to_string()])
        );
    }

    #[test]
    fn build_create_config_copies_healthcheck() {
        let mut run_cfg = default_run_cfg();
        run_cfg.healthcheck = Some(bollard::models::HealthConfig {
            test: Some(vec!["CMD-SHELL".to_string(), "exit 0".to_string()]),
            retries: Some(2),
            ..Default::default()
        });
        let cfg = build_create_config(&run_cfg, "img:latest", None);
        let hc = cfg.healthcheck.expect("healthcheck should be set on body");
        assert_eq!(
            hc.test,
            Some(vec!["CMD-SHELL".to_string(), "exit 0".to_string()])
        );
        assert_eq!(hc.retries, Some(2));
    }

    #[test]
    fn build_create_config_copies_mounts() {
        let mut run_cfg = default_run_cfg();
        run_cfg.mounts = Some(vec![bollard::models::Mount {
            target: Some("/data".to_string()),
            source: Some("myvolume".to_string()),
            typ: Some(bollard::models::MountTypeEnum::VOLUME),
            ..Default::default()
        }]);
        let cfg = build_create_config(&run_cfg, "img:latest", None);
        let mounts = cfg
            .host_config
            .unwrap()
            .mounts
            .expect("mounts should be set");
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].target, Some("/data".to_string()));
    }

    #[test]
    fn build_create_config_copies_volumes() {
        let mut run_cfg = default_run_cfg();
        run_cfg.volumes = Some(vec!["/data".to_string()]);
        let cfg = build_create_config(&run_cfg, "img:latest", None);
        assert_eq!(cfg.volumes, Some(vec!["/data".to_string()]));
    }

    // ── extract_run_config / build_create_config — medium-priority bug fixes ────

    #[test]
    fn extract_run_config_copies_security_opt() {
        let inspect = bollard::models::ContainerInspectResponse {
            host_config: Some(bollard::models::HostConfig {
                security_opt: Some(vec![
                    "apparmor=my-profile".to_string(),
                    "seccomp=unconfined".to_string(),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        assert_eq!(
            run_cfg.security_opt,
            Some(vec![
                "apparmor=my-profile".to_string(),
                "seccomp=unconfined".to_string(),
            ])
        );
    }

    #[test]
    fn extract_run_config_copies_resource_limits() {
        let inspect = bollard::models::ContainerInspectResponse {
            host_config: Some(bollard::models::HostConfig {
                memory: Some(536_870_912),
                memory_swap: Some(1_073_741_824),
                memory_reservation: Some(268_435_456),
                nano_cpus: Some(500_000_000),
                cpu_shares: Some(512),
                cpu_period: Some(100_000),
                cpu_quota: Some(50_000),
                cpuset_cpus: Some("0-3".to_string()),
                cpuset_mems: Some("0".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        assert_eq!(run_cfg.memory, Some(536_870_912));
        assert_eq!(run_cfg.memory_swap, Some(1_073_741_824));
        assert_eq!(run_cfg.memory_reservation, Some(268_435_456));
        assert_eq!(run_cfg.nano_cpus, Some(500_000_000));
        assert_eq!(run_cfg.cpu_shares, Some(512));
        assert_eq!(run_cfg.cpu_period, Some(100_000));
        assert_eq!(run_cfg.cpu_quota, Some(50_000));
        assert_eq!(run_cfg.cpuset_cpus, Some("0-3".to_string()));
        assert_eq!(run_cfg.cpuset_mems, Some("0".to_string()));
    }

    #[test]
    fn extract_run_config_copies_tmpfs() {
        let mut tmpfs = HashMap::new();
        tmpfs.insert("/run".to_string(), "size=64m".to_string());
        let inspect = bollard::models::ContainerInspectResponse {
            host_config: Some(bollard::models::HostConfig {
                tmpfs: Some(tmpfs.clone()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        assert_eq!(run_cfg.tmpfs, Some(tmpfs));
    }

    #[test]
    fn extract_run_config_copies_dns_settings() {
        let inspect = bollard::models::ContainerInspectResponse {
            host_config: Some(bollard::models::HostConfig {
                dns: Some(vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()]),
                dns_search: Some(vec!["example.com".to_string()]),
                dns_options: Some(vec!["ndots:2".to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        assert_eq!(
            run_cfg.dns,
            Some(vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()])
        );
        assert_eq!(run_cfg.dns_search, Some(vec!["example.com".to_string()]));
        assert_eq!(run_cfg.dns_options, Some(vec!["ndots:2".to_string()]));
    }

    #[test]
    fn extract_run_config_copies_runtime() {
        let inspect = bollard::models::ContainerInspectResponse {
            host_config: Some(bollard::models::HostConfig {
                runtime: Some("nvidia".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        assert_eq!(run_cfg.runtime, Some("nvidia".to_string()));
    }

    #[test]
    fn build_create_config_copies_security_opt() {
        let mut run_cfg = default_run_cfg();
        run_cfg.security_opt = Some(vec!["seccomp=unconfined".to_string()]);
        let cfg = build_create_config(&run_cfg, "img:latest", None);
        assert_eq!(
            cfg.host_config.unwrap().security_opt,
            Some(vec!["seccomp=unconfined".to_string()])
        );
    }

    #[test]
    fn build_create_config_copies_resource_limits() {
        let mut run_cfg = default_run_cfg();
        run_cfg.memory = Some(536_870_912);
        run_cfg.nano_cpus = Some(500_000_000);
        run_cfg.cpu_shares = Some(512);
        run_cfg.cpu_quota = Some(50_000);
        run_cfg.cpuset_cpus = Some("0-1".to_string());
        let cfg = build_create_config(&run_cfg, "img:latest", None);
        let hc = cfg.host_config.unwrap();
        assert_eq!(hc.memory, Some(536_870_912));
        assert_eq!(hc.nano_cpus, Some(500_000_000));
        assert_eq!(hc.cpu_shares, Some(512));
        assert_eq!(hc.cpu_quota, Some(50_000));
        assert_eq!(hc.cpuset_cpus, Some("0-1".to_string()));
    }

    #[test]
    fn build_create_config_copies_tmpfs() {
        let mut run_cfg = default_run_cfg();
        let mut tmpfs = HashMap::new();
        tmpfs.insert("/run".to_string(), "size=64m".to_string());
        run_cfg.tmpfs = Some(tmpfs.clone());
        let cfg = build_create_config(&run_cfg, "img:latest", None);
        assert_eq!(cfg.host_config.unwrap().tmpfs, Some(tmpfs));
    }

    #[test]
    fn build_create_config_copies_dns_settings() {
        let mut run_cfg = default_run_cfg();
        run_cfg.dns = Some(vec!["1.1.1.1".to_string()]);
        run_cfg.dns_search = Some(vec!["example.com".to_string()]);
        run_cfg.dns_options = Some(vec!["ndots:2".to_string()]);
        let cfg = build_create_config(&run_cfg, "img:latest", None);
        let hc = cfg.host_config.unwrap();
        assert_eq!(hc.dns, Some(vec!["1.1.1.1".to_string()]));
        assert_eq!(hc.dns_search, Some(vec!["example.com".to_string()]));
        assert_eq!(hc.dns_options, Some(vec!["ndots:2".to_string()]));
    }

    #[test]
    fn build_create_config_copies_runtime() {
        let mut run_cfg = default_run_cfg();
        run_cfg.runtime = Some("nvidia".to_string());
        let cfg = build_create_config(&run_cfg, "img:latest", None);
        assert_eq!(cfg.host_config.unwrap().runtime, Some("nvidia".to_string()));
    }

    // ── extract_run_config / build_create_config — lower-priority bug fixes ─────

    #[test]
    fn extract_run_config_copies_sysctls() {
        let mut sysctls = HashMap::new();
        sysctls.insert("net.ipv4.ip_forward".to_string(), "1".to_string());
        let inspect = bollard::models::ContainerInspectResponse {
            host_config: Some(bollard::models::HostConfig {
                sysctls: Some(sysctls.clone()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(extract_run_config(&inspect).sysctls, Some(sysctls));
    }

    #[test]
    fn extract_run_config_copies_pid_ipc_userns_mode() {
        let inspect = bollard::models::ContainerInspectResponse {
            host_config: Some(bollard::models::HostConfig {
                pid_mode: Some("host".to_string()),
                ipc_mode: Some("shareable".to_string()),
                userns_mode: Some("host".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let run_cfg = extract_run_config(&inspect);
        assert_eq!(run_cfg.pid_mode, Some("host".to_string()));
        assert_eq!(run_cfg.ipc_mode, Some("shareable".to_string()));
        assert_eq!(run_cfg.userns_mode, Some("host".to_string()));
    }

    #[test]
    fn extract_run_config_copies_readonly_rootfs() {
        let inspect = bollard::models::ContainerInspectResponse {
            host_config: Some(bollard::models::HostConfig {
                readonly_rootfs: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(extract_run_config(&inspect).readonly_rootfs, Some(true));
    }

    #[test]
    fn extract_run_config_copies_pids_limit() {
        let inspect = bollard::models::ContainerInspectResponse {
            host_config: Some(bollard::models::HostConfig {
                pids_limit: Some(100),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(extract_run_config(&inspect).pids_limit, Some(100));
    }

    #[test]
    fn build_create_config_copies_pid_ipc_userns_mode() {
        let mut run_cfg = default_run_cfg();
        run_cfg.pid_mode = Some("host".to_string());
        run_cfg.ipc_mode = Some("shareable".to_string());
        run_cfg.userns_mode = Some("host".to_string());
        let hc = build_create_config(&run_cfg, "img:latest", None)
            .host_config
            .unwrap();
        assert_eq!(hc.pid_mode, Some("host".to_string()));
        assert_eq!(hc.ipc_mode, Some("shareable".to_string()));
        assert_eq!(hc.userns_mode, Some("host".to_string()));
    }

    #[test]
    fn build_create_config_copies_sysctls_readonly_pids_limit() {
        let mut run_cfg = default_run_cfg();
        let mut sysctls = HashMap::new();
        sysctls.insert("net.ipv4.ip_forward".to_string(), "1".to_string());
        run_cfg.sysctls = Some(sysctls.clone());
        run_cfg.readonly_rootfs = Some(true);
        run_cfg.pids_limit = Some(100);
        let hc = build_create_config(&run_cfg, "img:latest", None)
            .host_config
            .unwrap();
        assert_eq!(hc.sysctls, Some(sysctls));
        assert_eq!(hc.readonly_rootfs, Some(true));
        assert_eq!(hc.pids_limit, Some(100));
    }

    // ── extract_docker_server_error ───────────────────────────────────────────

    #[test]
    fn extract_docker_server_error_returns_status_and_message() {
        let e = anyhow::anyhow!(bollard::errors::Error::DockerResponseServerError {
            status_code: 409,
            message: "conflict: image is in use by container abc".to_string(),
        });
        assert_eq!(
            extract_docker_server_error(&e),
            Some((
                409,
                "conflict: image is in use by container abc".to_string()
            ))
        );
    }

    #[test]
    fn extract_docker_server_error_returns_none_for_other_bollard_errors() {
        let e = anyhow::anyhow!(bollard::errors::Error::RequestTimeoutError);
        assert!(extract_docker_server_error(&e).is_none());
    }

    #[test]
    fn extract_docker_server_error_returns_none_for_non_bollard_errors() {
        let e = anyhow::anyhow!("something went wrong");
        assert!(extract_docker_server_error(&e).is_none());
    }

    // ── SessionReport::record ─────────────────────────────────────────────────

    #[test]
    fn session_report_records_updated() {
        let mut report = SessionReport::default();
        report.record(
            "nginx",
            &UpdateResult::Updated {
                old_image: "nginx:1.0".to_string(),
                old_digest: "sha256:aaa".to_string(),
                new_image: "nginx:2.0".to_string(),
                new_digest: "sha256:bbb".to_string(),
            },
            None,
        );
        assert_eq!(report.containers.len(), 1);
        assert_eq!(report.containers[0].outcome, ContainerOutcome::Updated);
        assert_eq!(report.containers[0].old_image.as_deref(), Some("nginx:1.0"));
        assert_eq!(report.containers[0].new_image.as_deref(), Some("nginx:2.0"));
    }

    #[test]
    fn session_report_records_skipped() {
        let mut report = SessionReport::default();
        report.record(
            "nginx",
            &UpdateResult::Skipped("monitor_only".to_string()),
            Some("nginx:1.0".to_string()),
        );
        assert_eq!(report.containers.len(), 1);
        assert_eq!(report.containers[0].outcome, ContainerOutcome::Skipped);
        assert_eq!(report.containers[0].old_image.as_deref(), Some("nginx:1.0"));
        assert!(report.containers[0].new_image.is_none());
    }

    #[test]
    fn session_report_records_failed() {
        let mut report = SessionReport::default();
        report.record(
            "nginx",
            &UpdateResult::Failed(anyhow::anyhow!("oops")),
            Some("nginx:1.0".to_string()),
        );
        assert_eq!(report.containers.len(), 1);
        assert_eq!(report.containers[0].outcome, ContainerOutcome::Failed);
        assert_eq!(report.containers[0].old_image.as_deref(), Some("nginx:1.0"));
        assert!(report.containers[0].new_image.is_none());
    }

    #[test]
    fn session_report_records_rolled_back() {
        let mut report = SessionReport::default();
        report.record(
            "nginx",
            &UpdateResult::RolledBack {
                old_image: "nginx:1.0".to_string(),
                old_digest: "sha256:aaa".to_string(),
                attempted_image: "nginx:2.0".to_string(),
                attempted_digest: "sha256:bbb".to_string(),
                reason: "healthcheck_failed".to_string(),
            },
            None,
        );
        assert_eq!(report.containers.len(), 1);
        assert_eq!(report.containers[0].outcome, ContainerOutcome::RolledBack);
        assert_eq!(report.containers[0].old_image.as_deref(), Some("nginx:1.0"));
        assert_eq!(report.containers[0].new_image.as_deref(), Some("nginx:2.0"));
    }

    #[test]
    fn session_report_records_up_to_date() {
        let mut report = SessionReport::default();
        report.record("nginx", &UpdateResult::UpToDate, None);
        assert_eq!(report.containers.len(), 1);
        assert_eq!(report.containers[0].outcome, ContainerOutcome::UpToDate);
        assert!(report.containers[0].old_image.is_none());
        assert!(report.containers[0].new_image.is_none());
    }

    // ── build_dependency_graph — Docker --link ────────────────────────────────

    #[test]
    fn dep_graph_docker_link_in_set() {
        let containers = vec![make_container("app"), make_container("redis")];
        let mut inspect_map: HashMap<String, bollard::models::ContainerInspectResponse> =
            HashMap::new();
        inspect_map.insert(
            "app".to_string(),
            bollard::models::ContainerInspectResponse {
                host_config: Some(bollard::models::HostConfig {
                    links: Some(vec!["/redis:/app/redis".to_string()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        let graph = build_dependency_graph(&containers, &inspect_map);
        assert!(graph["app"].contains(&"redis".to_string()));
        assert!(graph["redis"].is_empty());
    }

    #[test]
    fn dep_graph_docker_link_outside_set_ignored() {
        let containers = vec![make_container("app")];
        let mut inspect_map: HashMap<String, bollard::models::ContainerInspectResponse> =
            HashMap::new();
        inspect_map.insert(
            "app".to_string(),
            bollard::models::ContainerInspectResponse {
                host_config: Some(bollard::models::HostConfig {
                    links: Some(vec!["/external:/app/ext".to_string()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        let graph = build_dependency_graph(&containers, &inspect_map);
        assert!(graph["app"].is_empty());
    }

    // ── local_image_is_newer ──────────────────────────────────────────────────

    #[test]
    fn local_image_newer_when_ids_differ() {
        assert!(local_image_is_newer("sha256:old", "sha256:new"));
    }

    #[test]
    fn local_image_not_newer_when_ids_match() {
        assert!(!local_image_is_newer("sha256:abc", "sha256:abc"));
    }

    #[test]
    fn local_image_not_newer_when_running_id_empty() {
        assert!(!local_image_is_newer("", "sha256:new"));
    }

    #[test]
    fn local_image_not_newer_when_local_id_empty() {
        assert!(!local_image_is_newer("sha256:old", ""));
    }
}
