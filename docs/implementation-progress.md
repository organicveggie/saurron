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

**Status:** Not started

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
