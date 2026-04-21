# CLAUDE.md

Guidance for Claude Code when working in this repo.

## Commands

```bash
# Build
cargo build
cargo build --profile release

# Test
cargo test
cargo test --profile release
cargo test <test_name>           # run a single test by name (substring match)
cargo test <module>::tests       # run all tests in a module

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
| `main.rs` | Wire everything: tracing init, config load, Docker connect, container enumeration, `UpdateEngine::run_cycle` |
| `cli.rs` | `clap`-derived `Args` struct — all flags with env var mappings (`SAURRON_*`) |
| `config.rs` | Layered merge: TOML file → env vars → CLI flags → built-in defaults; Docker-secrets resolution (file path → file contents) |
| `docker.rs` | Bollard wrapper: connection, container listing/selection, label parsing, image inspect/pull/stop/remove/create/start |
| `registry.rs` | Docker Registry HTTP API v2 client: manifest HEAD for digests, tag listing, Bearer auth, SemVer ranking |
| `update.rs` | Core engine: `run_cycle` orchestrate freshness checks → dependency sort → per-container pull/restart/rollback |
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

UpdateEngine::run_cycle  ← update.rs
  Phase A: check_freshness for each container
  Phase B: inspect stale containers → capture ContainerRunConfig
  Phase C: topological_sort (Kahn's algorithm, dependents-first)
  Phase D: for each stale container → pull → stop → remove → create → start → health check → audit
  Phase E: session summary / report
```

### Container selection

`ContainerSelector` in `docker.rs` — two modes:

- **Opt-out** (default): all running containers included unless explicitly disabled
- **Opt-in** (`label_enable = true`): only containers with `saurron.enable=true` label

`global_takes_precedence` flag control whether global config override per-container labels. `resolve_bool_override()` in `update.rs` encode this logic, shared across multiple settings.

### Per-container labels

Labels on container (prefix `saurron.`) override global config:
`enable`, `monitor-only`, `no-pull`, `stop-signal`, `stop-timeout`, `depends-on`, `semver-pre-release`

### Registry authentication

`RegistryClient` negotiate Bearer tokens on 401. Docker Hub use separate OAuth flow (`fetch_docker_hub_token`). Credentials flow from `Config::registry_username/password` — values may be Docker secret file paths resolved at startup.

### Dependency ordering

`build_dependency_graph` construct `HashMap<name, Vec<dep_names>>` from three sources: `saurron.depends-on` labels, Docker `--link` flags, `network_mode: container:<name>`. `topological_sort` apply Kahn's algorithm **dependents-first** (if A depends on B, A updated before B). Cycles appended last with warning.

## Testing notes

- Unit tests live inline in each module under `#[cfg(test)] mod tests`.
- `registry.rs` include property-based tests using `proptest` in nested `mod proptests`.
- One `#[tokio::test]` in `registry.rs` for Docker Hub auth path; all other tests synchronous.
- `docker.rs` tests extensive for pure data-transformation functions (label parsing, container selection); `DockerClient` methods calling Bollard have no tests (need live Docker socket or mocking).

## Code Style

### Rust

- Run `rustfmt --edition 2024` on Rust files after modifying
- Keep code coverage at or above 40% for new code

### Markdown

* Always place blank line before first line in list (ordered or unordered).