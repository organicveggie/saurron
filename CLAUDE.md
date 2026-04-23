# CLAUDE.md

Guidance for Claude Code when working in this repo.

## Commands

```bash
# Build
cargo build
cargo build --profile release

# Test
cargo test                                               # unit tests only (fast)
cargo test --profile release
cargo test <test_name>                                   # run a single test by name (substring match)
cargo test <module>::tests                               # run all tests in a module
cargo test --test integration -- --include-ignored       # run integration tests (needs Docker socket)

# Lint
cargo clippy
cargo fmt --check

# Coverage
cargo tarpaulin --ignore-tests -o Html -o Lcov --timeout 240 --engine llvm
```

Build script (`build.rs`) embed version string from `SAURRON_BUILD_VERSION` env var (default `v0.0.0-unknown`). Access at runtime via `SAURRON_VERSION` compile-time env var.

## Architecture

Saurron = Docker container auto-updater. Watch containers, check image registries for updates, restart containers with newer image — preserve original run config.

### Module overview

| File | Role |
|---|---|
| `lib.rs` | Library crate root — re-exports all modules as `pub mod`; enables `tests/` integration tests to import them |
| `main.rs` | Binary entry point: tracing init, config load, Docker connect, container enumeration, scheduler + HTTP API wiring |
| `cli.rs` | `clap`-derived `Args` struct — all flags with env var mappings (`SAURRON_*`) |
| `config.rs` | Layered merge: TOML file → env vars → CLI flags → built-in defaults; Docker-secrets resolution (file path → file contents) |
| `docker.rs` | Bollard wrapper: connection, container listing/selection, label parsing, image inspect/pull/stop/remove/create/start/rename |
| `registry.rs` | Docker Registry HTTP API v2 client: manifest HEAD for digests, tag listing, Bearer auth, SemVer ranking; HTTP used for localhost/127.0.0.1 |
| `update.rs` | Core engine: `run_cycle` orchestrate freshness checks → dependency sort → per-container pull/restart/rollback; self-update deferred to end of cycle |
| `scheduler.rs` | `ScheduleMode` enum (`RunOnce`/`Interval`/`Cron`) and `run_scheduler` generic loop; validates mutual exclusion of `--run-once`/`--interval`/`--schedule` |
| `http.rs` | Axum HTTP API: `POST /v1/update`, `GET /v1/health`, `GET /v1/metrics`; Bearer token auth; `AppStateInner` shared state; update-lock prevents concurrent cycles |
| `selfupdate.rs` | Detect own container ID from `$HOSTNAME`/`/etc/hostname`; naming helpers for self-update rename flow |
| `audit.rs` | Two thin functions (`audit_update`, `audit_rollback`) emit structured events to dedicated tracing target |

### Data flow

```
CLI args + TOML + env vars
        │
        ▼
   Config::load()          ← config.rs
        │
        ├──► DockerClient  ← docker.rs  (Bollard, socket or TLS/TCP)
        │         │
        │         └── list_containers → ContainerSelector → Vec<ContainerInfo>
        │
        └──► RegistryClient ← registry.rs  (reqwest + Bearer auth)
                  │
                  └── check_freshness → FreshnessResult (UpToDate|Stale|Skipped|Error)

AppStateInner (Arc)  ← http.rs
  ├── DockerClient
  ├── RegistryClient
  ├── Config
  ├── ContainerSelector
  └── update_lock (Mutex)

main.rs dispatch:
  --run-once  → run_cycle_with_state() directly, exit
  otherwise   → scheduler task (tokio::spawn) + optional HTTP server
               tokio::select! { scheduler | http_server | SIGTERM/SIGINT }

UpdateEngine::run_cycle  ← update.rs
  Phase A: check_freshness for each container
  Phase B: inspect stale containers → capture ContainerRunConfig
  Phase C: topological_sort (Kahn's algorithm, dependents-first)
  Phase D: for each stale container → pull → stop → remove → create → start → health check → audit
  Phase D2: self-update (own container, deferred to last)
  Phase E: session summary / report
```

### Container selection

`ContainerSelector` in `docker.rs` — two modes:

- **Opt-out** (default): all running containers included unless explicitly disabled
- **Opt-in** (`label_enable = true`): only containers with `saurron.enable=true` label

`global_takes_precedence` flag control whether global config override per-container labels. `resolve_bool_override()` in `update.rs` encode this logic, shared across multiple settings.

### Per-container labels

Labels on container (prefix `saurron.`) override global config:
`enable`, `monitor-only`, `no-pull`, `stop-signal`, `stop-timeout`, `depends-on`, `semver-pre-release`, `non-semver-strategy`, `image-tag`, `scope`

### Registry authentication

`RegistryClient` negotiate Bearer tokens on 401. Docker Hub use separate OAuth flow (`fetch_docker_hub_token`). Credentials flow from `Config::registry_username/password` — values may be Docker secret file paths resolved at startup. `localhost` and `127.0.0.1` registries use plain HTTP (not HTTPS).

### Dependency ordering

`build_dependency_graph` construct `HashMap<name, Vec<dep_names>>` from three sources: `saurron.depends-on` labels, Docker `--link` flags, `network_mode: container:<name>`. `topological_sort` apply Kahn's algorithm **dependents-first** (if A depends on B, A updated before B). Cycles appended last with warning.

### HTTP API

Enabled only when `--http-api-update` or `--http-api-metrics` is set.

- `POST /v1/update` — triggers immediate update cycle; supports `?container=` and `?image=` scope filters; returns JSON `SessionReport`; requires Bearer token; 409 if cycle already running
- `GET /v1/health` — always returns 200; no auth
- `GET /v1/metrics` — Prometheus text format; Bearer token (exempt with `--http-api-metrics-no-auth`)

Lock semantics: scheduler uses `.lock().await` (waits); HTTP uses `.try_lock()` (returns 409) — prevents double-cycle overlap.

### Self-update

When Saurron detects its own container is stale, it: pulls new image → renames self to `<name>-saurron-old` → starts replacement under original name. On failure: stops replacement, renames self back, returns `Failed`. Processed last in `run_cycle` so all other containers update first.

## Testing notes

- Unit tests live inline in each module under `#[cfg(test)] mod tests`; run with `cargo test` (fast, no Docker needed).
- Integration tests live in `tests/integration.rs`; all marked `#[ignore]` — run with `cargo test --test integration -- --include-ignored` (requires live Docker socket and pulls `registry:2`, `busybox`, `alpine` from Docker Hub).
- `registry.rs` includes property-based tests using `proptest` in nested `mod proptests`.
- Async tests: `registry.rs` (Docker Hub auth path), `scheduler.rs` (`run_once_calls_cycle_exactly_once`, `interval_loop_executes_cycle`), all integration tests.
- `docker.rs` tests extensive for pure data-transformation functions (label parsing, container selection); `DockerClient` methods calling Bollard have no unit tests (need live Docker socket or mocking) — covered by integration tests.
- Coverage target: ≥ 40% (currently ~42%). `cargo tarpaulin --ignore-tests` measures lib crate only.

## Code Style

### Rust

- Run `rustfmt --edition 2024` on Rust files after modifying
- Keep code coverage at or above 40% for new code

### Markdown

* Always place blank line before first line in list (ordered or unordered).
