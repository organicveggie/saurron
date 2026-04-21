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

**Status:** Not started

---

## Phase 6 — Rollback manager

**Status:** Not started

---

## Phase 7 — Scheduler & HTTP API

**Status:** Not started

---

## Phase 8 — Self-update & graceful shutdown

**Status:** Not started

---

## Phase 9 — Notification system

**Status:** Not started

---

## Phase 10 — Prometheus metrics

**Status:** Not started

---

## Phase 11 — Containerization & integration testing

**Status:** Not started
