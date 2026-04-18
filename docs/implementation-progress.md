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

**Status:** In progress

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

---

## Phase 3 — Structured logging & audit trail

**Status:** Not started

---

## Phase 4 — Registry client & freshness detection

**Status:** Not started

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
