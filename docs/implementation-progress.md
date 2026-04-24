# Saurron — Implementation Progress

## Phase 1 — Project scaffold & configuration layer

**Status:** Complete

**Completed work:**

- `Cargo.toml` — all §10 deps declared; `proptest` as dev-dep; `build = "build.rs"`
- `build.rs` — reads `SAURRON_BUILD_VERSION` at compile time; injects as `SAURRON_VERSION` (fallback: `v0.0.0-unknown`)
- `src/cli.rs` — `clap` `Args` struct covering every §11 flag; all optional fields typed `Option<T>` so absent flags are `None` (allows TOML to win); `LogLevel`, `LogFormat`, `HeadWarnStrategy` enums with `ValueEnum` derive; `--debug`/`--trace` shorthands; `--no-rollback-on-*` paired flags
- `src/config.rs` — `Config` + nested concrete structs (`DockerConfig`, `RollbackConfig`, `HttpApiConfig`, `NotificationsConfig` etc.); `PartialConfig` all-`Option` hierarchy for TOML deserialization; `Config::load(&Args)` merges: `config-rs` TOML → clap `Option<T>` (env/CLI) → built-in defaults; `resolve_secrets()` replaces designated fields with file contents when value is a readable path (Docker secrets); 4 unit tests
- `src/main.rs` — parse args → init `tracing_subscriber::fmt()` with level from args (INFO default) → load full config → `info!` startup log with version

**Milestone verification:** `cargo build` succeeds; `saurron --help` shows all flags; config loads from TOML, env vars, CLI with correct precedence.

**Notes:**
- Log format config (json/logfmt/auto) is wired in Phase 3; Phase 1 always uses pretty format
- TOML log level can only take effect if config is loaded before tracing init; full resolution deferred to Phase 3

---

## Phase 2 — Docker client & container enumeration

**Status:** Complete

---

### Step 1 — Docker client module (connection)

**Status:** Complete

**Completed work:**

- `Cargo.toml` — `bollard` updated to `{ version = "0.17", features = ["ssl"] }`; bollard's `ssl` feature uses rustls internally (no OpenSSL dependency)
- `src/docker.rs` (new) — `DockerClient` wrapping `bollard::Docker`; `ConnectionType` enum (`Socket`/`Http`/`Https`); `connection_type(host, tls_verify)` pure fn for scheme detection; `parse_api_version()` pure fn supporting `"1.44"` and `"v1.44"` formats; `DockerClient::connect(config)` builds the bollard connection (Unix socket via `connect_with_socket`, plain TCP via `connect_with_http`, TLS via `connect_with_ssl` with file-path cert arguments); `DockerClient::ping()` async; 11 unit tests covering all connection type cases and API version parsing
- `src/main.rs` — `mod docker` declared; converted to `#[tokio::main] async fn main`; calls `DockerClient::connect` + `.ping()` on startup and logs success

**Notes:**
- `bollard::ssl` feature gates `connect_with_ssl`, which takes cert/key/CA as `&Path` directly — no need for `rustls`/`rustls-pemfile` as direct dependencies
- Server-only TLS (no client cert) is supported: passing empty `PathBuf` for key/cert causes bollard's `DockerClientCertResolver` to return `None`, skipping client auth
- Hardcoded 120s connect timeout in all `connect_with_*` calls; tracked as a TODO to make this a `DockerConfig` field at end of Phase 2

### Step 2 — Container data model & label parsing

**Status:** Complete

**Completed work:**

- `src/docker.rs` — added `use std::collections::HashMap`
- `ContainerState` enum (`Created`/`Restarting`/`Running`/`Removing`/`Paused`/`Exited`/`Dead`/`Unknown(String)`) with `from_str()` and `Display` impl
- `ContainerInfo` struct (`id`, `name`, `image`, `image_id`, `state: ContainerState`, `labels: HashMap<String, String>`) with `.saurron_labels()` convenience method
- `SaurronLabels` struct (`enable: Option<bool>`, `scope: Option<String>`, `depends_on: Vec<String>`, `image_tag: Option<String>`) with `from_labels()`, `Default` derive; label keys: `saurron.enable`, `saurron.scope`, `saurron.depends-on`, `saurron.image-tag`
- `parse_bool_label()` — case-insensitive, accepts only `"true"`/`"false"`, returns `None` for anything else
- `parse_depends_on()` — splits on comma, trims whitespace, filters empty entries
- 23 new unit tests (34 total): all `ContainerState` variants, `Display`, and full edge-case coverage of `SaurronLabels::from_labels()`

### Step 3 — State filtering & selection logic

**Status:** Complete

**Completed work:**

- `src/docker.rs` — added `use std::collections::HashSet`
- `ContainerSelector` struct with fields: `label_enable`, `global_takes_precedence`, `disabled_names: HashSet<String>`, `allowed_names: Option<HashSet<String>>`, `include_restarting`, `revive_stopped`
- `state_filter()` — returns `["running"]` base, appends `"restarting"` if `--include-restarting`, appends `"exited"`/`"created"` if `--revive-stopped`
- `is_selected()` — first checks `allowed_names` allow-list (if set), then hard-excludes by `disabled_names`, then: opt-in mode (`label_enable=true`) requires `saurron.enable=true`; opt-out + `global_takes_precedence=true` ignores per-container disable labels; opt-out default excludes only containers with `saurron.enable=false`
- `select()` — filters a `&[ContainerInfo]` slice, returning owned `Vec<ContainerInfo>`
- 25 unit tests (59 total in docker.rs): state filter combinations, opt-in/opt-out behaviour, `disable_containers` hard-exclude, `global_takes_precedence` override, `select()` end-to-end, and 6 new `allowed_names` tests

### Step 4 — Enumeration & wire to main (milestone)

**Status:** Complete

**Completed work:**

- `src/docker.rs` — `summary_to_info()` private fn mapping `bollard::models::ContainerSummary` → `ContainerInfo` (strips leading `/` from Docker name, handles all `Option` fields gracefully, skips rows with no `id`)
- `DockerClient::list_containers(selector)` — calls bollard `list_containers` with `all: true` and a `status` filter built from `selector.state_filter()`; returns `Vec<ContainerInfo>`
- `DockerClient::select_containers(containers, selector)` — delegates to `selector.select()`; thin wrapper for ergonomic call site in `main.rs`
- `src/main.rs` — builds `ContainerSelector` from `Config`; calls `list_containers` → `select_containers`; logs summary count line + one `info!` line per selected container (id, name, image, state)

**Milestone verification:** Binary enumerates containers on live Docker daemon, applies all inclusion/exclusion rules, prints structured list.

---

## Phase 3 — Structured logging & audit trail

**Status:** Complete

**Completed work:**

- `Cargo.toml` — added `tracing-appender = "0.2"` and `tracing-logfmt = "0.3"`; added `"json"` feature to `tracing-subscriber`
- `src/audit.rs` (new) — `audit_update()` and `audit_rollback()` functions emit structured events with `target: "saurron::audit"`; all fields match the audit trail spec (container name/id, old/new image tag+digest, outcome, failure reason); call sites in Phases 5 and 6
- `src/main.rs` — reordered init to `parse args → load config → init tracing` so TOML log settings apply from the first log line; replaced Phase 1 stub with `init_tracing()` supporting all four formats: `pretty` (colored human-readable), `json` (newline-delimited JSON), `logfmt` (key=value pairs), `auto` (pretty on TTY, logfmt when piped); boxed layers collected into `Vec` and added via single `.with()` to satisfy tracing-subscriber's type constraints; `EnvFilter` wraps outer layer so `RUST_LOG` env var overrides config-derived level; optional audit file layer wired with `tracing_appender::rolling::never()` + `non_blocking()`, filtered to `saurron::audit` target only; `WorkerGuard` held in `_guard` for full program lifetime to ensure flush on exit; parent directory created with `create_dir_all` if absent

**Milestone verification:** JSON logs flow to stdout with `--log-format json`; logfmt format on non-TTY with `--log-format auto`; audit events flow to configured file when `--audit-log` is set.

**Notes:**
- `audit_update()` and `audit_rollback()` are defined but have no call sites yet; call sites added in Phases 5 and 6 respectively
- `tracing_logfmt::layer()` returns `tracing_subscriber::fmt::Layer` (writes to stdout by default); `BoxLayer` type alias is `Box<dyn Layer<Registry> + Send + Sync>`

---

## Phase 4 — Registry client & freshness detection

**Status:** Complete

**Completed work:**

- `src/registry.rs` (new) — full Docker Registry HTTP API v2 client:
  - `parse_image_ref()` — parses any image ref form: official library images, namespaced, custom registry (with port), `docker.io` normalisation to `registry-1.docker.io`, digest-pinned (`@sha256:`) detection, default `latest` tag
  - `parse_semver_tag()` — strip optional `v` prefix, parse strict SemVer 2.0.0 with `semver` crate; non-SemVer tags (e.g., `latest`, `1.25`) return `None`
  - `find_best_semver_update()` — highest version strictly greater than current; pre-release gated by `allow_prerelease` flag
  - `parse_non_semver_strategy()` — maps label value `"skip"` / `"digest"` to `NonSemverStrategy` enum
  - `RegistryClient::new()` — 30s timeout reqwest client; `User-Agent: saurron/<version>`
  - `RegistryClient::check_freshness()` — dispatches: digest-pinned → skip; SemVer → tag enumeration + version comparison; non-SemVer → manifest digest HEAD comparison
  - Bearer token auth: parses `WWW-Authenticate: Bearer realm=...,service=...,scope=...`; fetches token; retries with `Authorization: Bearer`; default pull scope when absent from header
  - `--head-warn-strategy` (`auto` / `always` / `never`) governs whether manifest fetch failures emit `warn!` or `debug!`
  - `FreshnessResult` enum: `UpToDate`, `Stale(StaleInfo)`, `Skipped(String)`, `Error(String)`
  - 36 unit tests: image ref parsing (10 cases), SemVer tag parsing (6), best-update selection (8), strategy parsing (4), well-known registry (3), WWW-Authenticate parsing (3)
  - 4 proptest properties: best result always > current; prerelease flag respected; arbitrary input never panics; known-valid refs always parse
- `src/docker.rs` — extended `SaurronLabels` with two new fields parsed from container labels:
  - `semver_pre_release: Option<bool>` from `saurron.semver-pre-release`
  - `non_semver_strategy: Option<String>` from `saurron.non-semver-strategy`
  - `DockerClient::get_image_manifest_digest(image)` — calls bollard `inspect_image`, extracts manifest digest from `RepoDigests` (`name@sha256:digest` format)
  - 6 new unit tests for new label fields (65 total in docker.rs)
- `src/main.rs` — freshness detection loop wired after container selection:
  - `RegistryClient` constructed at startup
  - Per-container: local manifest digest fetched from Docker; `check_freshness()` called; results logged; inspect failures degrade gracefully (log warn, continue with no digest)
  - `Scan complete` log line with stale count, total, and monitor-only flag

**Milestone verification:** `saurron --monitor-only --log-format pretty` connects to Docker, enumerates containers, checks each against its registry, logs stale/up-to-date/skipped results, exits cleanly. All 97 unit and property-based tests pass.

---

## Post-Phase 4 Enhancement — `--containers` allow-list flag

**Status:** Complete

**Completed work:**

- `src/cli.rs` — added `--containers` / `SAURRON_CONTAINERS` flag accepting a comma-separated list of container names
- `src/config.rs` — added `containers: Vec<String>` to `Config` and `PartialConfig`; wired through `merge()`
- `src/docker.rs` — added `allowed_names: Option<HashSet<String>>` to `ContainerSelector`; `new()` accepts a `containers: &[String]` parameter and sets `allowed_names` to `Some(set)` when non-empty, `None` otherwise; `is_selected()` checks allow-list first before all other filters; 6 new unit tests covering: empty slice → no restriction, matching name → included, non-matching → excluded, `disable_containers` overlap → disabled wins, `label_enable` interaction → label check still applies, `select()` end-to-end
- `src/main.rs` — passes `&config.containers` to `ContainerSelector::new()`

---

## Phase 5 — Update engine (happy path)

**Status:** Complete

**Completed work:**

- `Cargo.toml` — added `futures = "0.3"` (required for consuming bollard's pull stream via `TryStreamExt`)
- `src/docker.rs` — extended `SaurronLabels` with four new per-container label fields (`monitor_only`, `no_pull`, `stop_signal`, `stop_timeout`) + label constants + `from_labels` arms + 10 new unit tests (75 total in docker.rs); added 7 new `DockerClient` async methods: `inspect_container`, `pull_image` (streams progress at trace level, bails on error items), `stop_container` (treats HTTP 304 as success), `remove_container`, `create_container` (returns new container ID), `start_container`, `remove_image`
- `src/update.rs` (new) — full update engine module:
  - `parse_duration_secs()` — parses `"10s"` / `"5m"` / `"1h"` / bare integer; 8 unit tests
  - `ContainerRunConfig` struct — owned snapshot of full container config (env, volumes, ports, networks, host_config fields) for recreating container with new image
  - `extract_run_config()` — maps `ContainerInspectResponse` → `ContainerRunConfig`
  - `build_create_config()` — maps `ContainerRunConfig` + new image → `bollard::container::Config<String>`; applies stop signal override
  - `parse_link_target()` — extracts Docker `--link` target name; 4 unit tests
  - `build_dependency_graph()` — builds `dep_graph[name] = [deps]` from `saurron.depends-on` labels, Docker `--link`, and `network_mode: container:<name>`; 3 unit tests
  - `topological_sort()` — Kahn's algorithm on reverse dependency graph (leaves-first; containers with no dependents update before containers that others depend on); cycle members appended at end with `warn!`; 4 unit tests
  - `UpdateResult` enum (`UpToDate` / `Skipped(String)` / `Updated { old_image, old_digest, new_image, new_digest }` / `Failed(anyhow::Error)`)
  - `SessionReport` struct — accumulates updated/skipped/failed/up_to_date counts
  - `resolve_bool_override()` — per-container label override resolution respecting `global_takes_precedence`; 3 unit tests
  - `UpdateEngine<'a>` — owns refs to `DockerClient`, `RegistryClient`, `Config`; derives credentials from config
  - `UpdateEngine::run_cycle()` — full cycle: freshness scan → inspect stale containers → topological sort → sequential update; logs session summary
  - `UpdateEngine::check_freshness()` — same logic as the former inline loop in main.rs
  - `UpdateEngine::update_one()` — happy-path update: pull → stop → remove → create → start → startup check (10s wait, warn if not running; rollback deferred to Phase 6) → audit trail → optional cleanup
- `src/main.rs` — replaced the inline freshness-check loop (lines 134–198) with `UpdateEngine::new(&docker, &registry_client, &config).run_cycle(&selected).await`

**Milestone verification:** All 140 unit and property-based tests pass. Binary compiles cleanly. `cargo run -- --run-once --monitor-only` still detects stale containers and exits. `cargo run -- --run-once` executes full update cycles with pull → stop → recreate → start → audit trail.

**Notes:**
- Phase 5 startup monitoring is intentionally simple: wait `min(startup_timeout, 10)` seconds then check `state.running`; if false, log warning and continue (no rollback). Full startup monitoring and rollback are Phase 6.
- `audit_rollback()` still has no call sites; wired in Phase 6.

---

## Phase 6 — Rollback manager

**Status:** Complete

**Completed work:**

- `src/update.rs` — full rollback manager wired into `update_one()`:
  - `RolledBack` variant added to `UpdateResult` (`old_image`, `old_digest`, `attempted_image`, `attempted_digest`, `reason: String`)
  - `rolled_back: Vec<String>` field added to `SessionReport`; `record()` match arm and Phase E summary log updated
  - `RollbackTrigger` enum (`NonZeroExit(i64)`, `HealthcheckFailure`, `StartupTimeout`) with `reason_str()` producing structured reason strings (`"exit_code=N"`, `"healthcheck_failed"`, `"startup_timeout"`)
  - `StartupEval` enum (`Ok`, `Rollback(RollbackTrigger)`, `Continue`) used by the pure per-poll decision function
  - `evaluate_startup_state()` — pure function over a `ContainerState` snapshot; checks exit code, healthcheck status, and running state to produce a `StartupEval`; unit-testable without async; respects `on_exit_code` and `on_healthcheck` flags
  - `monitor_startup()` — async polling loop (1s interval) up to `startup_timeout`; calls `evaluate_startup_state` each tick; returns `Ok(())` on success or `Err(RollbackTrigger)` on failure; transient inspect errors are logged and skipped; if deadline passed and `on_timeout=false`, returns `Ok` rather than blocking indefinitely
  - Step 8 of `update_one()` replaced: on `Err(trigger)` from `monitor_startup`, stops and removes the new container, recreates and starts the old container from the preserved `run_cfg` + `old_image`, calls `audit::audit_rollback()`, returns `UpdateResult::RolledBack`; old image is never removed on rollback (cleanup step skipped by early return); rollback failure (create or start error) returns `UpdateResult::Failed` with context
  - `run_cycle()` Phase D updated to log rolled-back containers at `warn!` level
  - 12 new unit tests: 9 cases for `evaluate_startup_state` covering all health/exit/running combinations, 3 cases for `RollbackTrigger::reason_str`

**Milestone verification:** All 182 unit and property-based tests pass. `cargo clippy` clean. `cargo fmt --check` clean.

**Notes:**
- `audit_rollback()` call site is in `update_one()` rollback branch; `audit_update()` call site is in the happy-path step 9 (unchanged from Phase 5)
- Rollback itself failing (create/start error) returns `Failed`, not `RolledBack` — operator intervention required in that case

---

## Post-Phase 6 Enhancement — Increased test coverage

**Status:** Complete

**Completed work:**

- Coverage increased from 38.7% to 45.7% (+7.0%) across 16 new tests:
  - `src/audit.rs` — smoke tests for `audit_update()` and `audit_rollback()` (previously 0% covered); now 4/4 lines
  - `src/config.rs` — `--no-rollback-on-healthcheck` and `--no-rollback-on-timeout` flag paths; email config construction (all 3 required fields present, and missing-field → `None`); MQTT config construction (broker + topic present, missing topic → `None`); Pushover config with both fields; webhook secret-file resolution via temp file; now 163/171 lines
  - `src/docker.rs` — `ContainerState::Display` for `Created`, `Restarting`, `Removing`, `Paused`, `Dead` (only `Running`, `Exited`, `Unknown` were previously tested); now 91/211 lines
  - `src/registry.rs` — `format_image_ref()`, `normalize_digest()`, `manifest_accept_header()` (three private pure functions with no prior test coverage); now 99/263 lines

---

## Phase 7 — Scheduler & HTTP API

**Status:** Complete

**Completed work:**

- `Cargo.toml` — added `cron = "0.12"` and `chrono = "0.4"` for cron expression parsing and datetime arithmetic
- `src/update.rs` — `parse_duration_secs` changed from `pub` to `pub(crate)`; `SessionReport` now derives `serde::Serialize` so HTTP handlers can return it as JSON
- `src/scheduler.rs` (new) — scheduling logic:
  - `ScheduleMode` enum: `RunOnce`, `Interval(Duration)`, `Cron(Box<cron::Schedule>)`
  - `parse_schedule_mode(config)` — validates mutual exclusion of `--run-once`/`--interval`/`--schedule` (catches TOML-file combinations not caught by clap `conflicts_with`); defaults to `Interval(86400s)` when no scheduling config is set
  - `run_scheduler<F, Fut>(mode, run_cycle)` — generic over a closure so the scheduler has no knowledge of `AppStateInner`; `RunOnce`: call once and return; `Interval`: run immediately then sleep; `Cron`: sleep until next trigger via `schedule.upcoming(Utc).next()` then run
  - 7 unit tests: `run_once_mode`, `interval_default_is_24h`, `interval_from_flag`, `interval_hours`, `cron_mode_from_flag`, `invalid_cron_expression_is_error`, `run_once_calls_cycle_exactly_once` (async)
- `src/http.rs` (new) — HTTP API server:
  - `AppStateInner` struct: owns `DockerClient`, `RegistryClient`, `Config`, `ContainerSelector`, and `tokio::sync::Mutex<()>` (update lock); `AppState = Arc<AppStateInner>`
  - `validate_token_config(cfg)` — fails fast at startup if `--http-api-update` is set without `--http-api-token`, or `--http-api-metrics` without token and without `--http-api-metrics-no-auth`
  - `check_auth(headers, token)` — extracts `Authorization: Bearer <value>` and compares with configured token
  - `run_cycle_with_state(state)` — shared async function used by both scheduler closure and HTTP handler; re-lists containers on every call to pick up Docker state changes
  - `GET /v1/health` — always available when server is running; returns 200 OK; no auth required
  - `POST /v1/update` — Bearer token auth; `try_lock()` returns 409 if a cycle is already running; supports `?container=<name>` and `?image=<ref>` query params (comma-separated) to scope which containers are updated; returns JSON `SessionReport`
  - `GET /v1/metrics` — Bearer token auth (exempt if `--http-api-metrics-no-auth`); returns Prometheus text format via `prometheus::TextEncoder`; metric values wired in Phase 10
  - `start_server(state)` — binds to `0.0.0.0:<port>`; conditionally registers `/v1/update` and `/v1/metrics` based on config flags; only starts when at least one API feature is enabled
  - 9 unit tests: 5 for `validate_token_config`, 4 for `check_auth`
- `src/main.rs` — restructured to use `Arc<AppStateInner>`:
  - `validate_token_config` and `parse_schedule_mode` called immediately after config load (fail fast before Docker connect)
  - `DockerClient`, `RegistryClient`, `ContainerSelector` moved into `Arc<AppStateInner>`; startup enumeration kept for logging only
  - `RunOnce`: calls `run_cycle_with_state` directly and exits
  - Polling/Cron: scheduler spawned as a `tokio::task`; scheduler acquires `.lock().await` before each cycle (waits behind any in-progress HTTP-triggered cycle); HTTP handler uses `.try_lock()` (returns 409 if scheduler is running); `tokio::select!` races HTTP server and scheduler task when HTTP API is enabled

**Milestone verification:** `cargo build` succeeds; all 198 unit and property-based tests pass; `cargo clippy` clean (no new warnings from Phase 7 code); `cargo fmt` clean.

**Notes:**

- `GET /v1/metrics` returns a valid (empty) Prometheus text response; Phase 10 wires the actual metric values
- The HTTP server starts only when `--http-api-update` or `--http-api-metrics` is set; `GET /v1/health` is always registered when the server runs
- Lock semantics: scheduler uses `.lock().await` (waits); HTTP POST uses `.try_lock()` (skip/409) — prevents double-cycle overlap while allowing HTTP to interrupt idle wait

---

## Phase 8 — Self-update & graceful shutdown

**Status:** Complete

**Completed work:**

- `src/docker.rs` — added `DockerClient::rename_container(id, new_name)` using `bollard::query_parameters::RenameContainerOptionsBuilder`
- `src/selfupdate.rs` (new) — container ID detection and naming utilities:
  - `detect_own_container_id()` — reads `$HOSTNAME` env var (Docker sets this to the first 12 chars of the container ID), falls back to `/etc/hostname`; returns `None` outside a container
  - `detect_own_container_id_inner(value, path)` — inner testable form that accepts the hostname value directly (avoids env var mutation in tests)
  - `temp_container_name(original)` — produces `"{original}-saurron-old"` as the rename target during self-update
  - `is_self_container(container_id, own_id)` — checks full ID == own_id or starts_with own_id (handles short 12-char vs full 64-char ID forms)
  - 9 unit tests: detection from value, detection from file, fallback to file on empty value, returns None when both missing, temp name suffix, exact/prefix/no-match for `is_self_container`
- `src/update.rs` — self-update integrated into `UpdateEngine`:
  - `run_cycle` now calls `selfupdate::detect_own_container_id()` at the start of each cycle
  - In Phase D, containers matching the own ID are deferred to a `self_update_queue` and processed last (after all other containers update)
  - `self_update_one(container, stale_info, inspect)` — self-update path: pull new image → extract run config → rename self to temp name → create new container under original name → start it → `monitor_startup` (same timeout and rollback flags as regular updates); on failure: stop + remove replacement container, rename self back to original name, return `UpdateResult::Failed`; on success: audit trail, return `UpdateResult::Updated`
- `src/main.rs` — graceful shutdown signal handling:
  - `shutdown_signal()` async function: on Linux waits for SIGTERM or SIGINT (via `tokio::signal::unix`); on other platforms waits for `ctrl_c`
  - Both the HTTP-enabled and no-HTTP dispatch paths now include a `shutdown_signal()` arm in `tokio::select!`; on receipt, acquires `update_lock.lock().await` (which blocks until any active update cycle finishes), logs completion, then exits
  - `--run-once` path is unaffected (single cycle, exits immediately)

**Milestone verification:** All 207 unit and property-based tests pass; `cargo clippy` clean (no new warnings from Phase 8 code); `cargo fmt` clean.

**Notes:**
- The self-update flow leaves the renamed old container alive after the replacement starts; the old container will stop naturally when the current process exits
- Self-update failure recovery restores the original container name via rename (no image recreate needed — the container is still running)
- "Flush pending notifications" on shutdown is a no-op until Phase 9 wires the notification system; the lock-based shutdown correctly waits for the current update cycle to complete

---

## Post-Phase 8 Enhancement — Initial Integration Tests

**Status:** Complete

**Completed work:**

- `src/lib.rs` (new) — declares all modules as `pub mod`; enables `tests/` to import from the library crate
- `src/main.rs` — removed nine inline `mod` declarations; now imports modules via `use saurron::{...}` from the library crate
- `src/http.rs` — promoted `AppStateInner`, its fields, `AppState`, `validate_token_config`, `run_cycle_with_state`, `start_server` from `pub(crate)` to `pub` (required by binary crate after lib extraction); `UpdateQuery` made private (not needed outside http.rs)
- `src/scheduler.rs` — promoted `ScheduleMode`, `parse_schedule_mode`, `run_scheduler` from `pub(crate)` to `pub`
- `src/registry.rs` — added private `scheme_for_registry(registry)` function that returns `"http"` for `localhost`/`127.0.0.1` and `"https"` otherwise; applied to both `fetch_manifest_digest` and `list_tags` URL construction; 2 unit tests
- `Cargo.toml` — added `testcontainers = "0.27"` to `[dev-dependencies]`
- `tests/integration.rs` (new) — four `#[tokio::test] #[ignore]` integration tests using a testcontainers-managed `registry:2` instance and the live Docker socket:
  - `docker_client_connect_and_ping` — `DockerClient::connect` + `.ping()` against real daemon
  - `registry_freshness_up_to_date` — push busybox to local registry; `check_freshness` with matching digest → `UpToDate`
  - `registry_freshness_stale_non_semver` — overwrite tag with alpine; old digest → `Stale`
  - `registry_freshness_semver_stale` — push `v1.0.0` and `v1.1.0`; check `v1.0.0` → `Stale` with `new_image` containing `v1.1.0`
- `.github/workflows/rust.yml` — added `cargo test --test integration -- --include-ignored` step after unit tests; both `ubuntu-latest` and `ubuntu-24.04-arm` runners have Docker available

**Notes:**

- Integration tests are `#[ignore]` by default so `cargo test` stays fast; run explicitly with `cargo test --test integration -- --include-ignored`
- HTTP support in `RegistryClient` is intentionally limited to `localhost`/`127.0.0.1`; on-prem HTTP-only registries at other addresses still use HTTPS (proper insecure-registry support deferred)
- 236 unit tests pass; binary still compiles cleanly after the lib extraction

---

## Phase 9 — Notification system

**Status:** Complete

**Completed work:**

- `Cargo.toml` — added lettre features: `tokio1-rustls-tls`, `builder`, `smtp-transport`, `ring` (consistent with the project's rustls-everywhere approach)
- `src/notifications.rs` (new) — all notification logic in one module:
  - `DEFAULT_TEMPLATE` — minijinja template constant with `updated`, `rolled_back`, `failed`, `up_to_date` variables; rendered once per cycle and shared across all configured targets
  - `should_notify(report)` — returns `true` only when `updated | failed | rolled_back` is non-empty (see `docs/TODO.md` for deferral note on making this configurable)
  - `render_template(report, template)` — pure function; accepts optional custom template string; uses `DEFAULT_TEMPLATE` when `None`; minijinja renders the `SessionReport` fields
  - `parse_webhook_headers(s)` — pure function; splits `"K:V,K2:V2"` on `,` then on first `:` (values may contain colons)
  - `parse_mqtt_broker(broker)` — parses `tcp://host:port`, `mqtt://host:port`, or bare `host:port` / `host` (defaults to port 1883)
  - `dispatch(config, report)` — async; checks `should_notify`, applies `GeneralNotifConfig.delay`, renders template, fans out to all four targets concurrently via `tokio::join!`; errors logged per-target, others continue
  - `send_webhook(cfg, body)` — reqwest `POST`; `Content-Type: text/plain`; custom headers; separate client with `danger_accept_invalid_certs` when `tls_skip_verify = true`
  - `send_email(cfg, body)` — lettre `AsyncSmtpTransport<Tokio1Executor>`; STARTTLS via `starttls_relay` or `builder_dangerous` + `TlsParameters` (skip-verify path); optional SMTP credentials; subject "Saurron update report"; `text/plain` body
  - `send_mqtt(cfg, body)` — rumqttc v5 `AsyncClient`; event loop driven in a spawned task; configurable QoS (0/1/2); optional credentials; auto-generated client ID when not configured; brief wait before disconnect
  - `send_pushover(cfg, body)` — reqwest JSON POST to `api.pushover.net/1/messages.json`; fields: `token`, `user`, `title`, `message`
  - 25 unit tests: template rendering (4), header parsing (5), `should_notify` (5), `parse_mqtt_broker` (4), `dispatch` no-op and error paths (3 async), `send_webhook` with local axum server (3 async)
- `src/lib.rs` — added `pub mod notifications;`
- `src/http.rs` — `run_cycle_with_state` now captures the `SessionReport` returned by `run_cycle` and calls `notifications::dispatch` after each cycle
- `tests/integration.rs` — added `webhook_dispatch_posts_to_local_server` (`#[ignore]`): starts a local axum server, renders a template, calls `send_webhook`, asserts the POST body matches
- `docs/TODO.md` — added notes on configurable notification trigger and MQTT TLS support

**Milestone verification:** All 261 unit tests pass; `cargo clippy` clean; `cargo fmt` clean; coverage 43.38% (above 42% threshold).

**Notes:**

- Notifications fire only on "interesting" cycles (any update, failure, or rollback); all-up-to-date cycles produce no notification — see `docs/TODO.md` for the deferred configurable-trigger item
- Pushover uses JSON body (not form encoding) since `reqwest` is configured with `default-features = false` and the `form` feature is not enabled
- MQTT is MQTTv5 (via `rumqttc-next = "0.30"` which re-exports `rumqttc_v5`); plain TCP only — TLS is a deferred TODO
- Email and MQTT senders have no unit tests (require live SMTP/broker); covered by the network-level integration path in Phase 11

---

## Phase 10 — Prometheus metrics

**Status:** Not started

---

## Phase 11 — Containerization & integration testing

**Status:** Not started
