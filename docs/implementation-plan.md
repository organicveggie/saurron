# Saurron — Implementation Plan

## Phase 1 — Project scaffold & configuration layer

**Work:**

- Init `Cargo.toml` with all deps from §10; use `thiserror` for domain errors in core modules, `anyhow` in `main` + top-level orchestration; add `proptest` as dev-dep
- Unit tests in `#[cfg(test)]` ship with each module; property-based tests added where noted in later phases
- Define full `Config` struct hierarchy (all §11 tables)
- Wire `clap` CLI, `config-rs` file/env layering, secret file resolution
- Build version injection via `build.rs`
- Wire minimal `tracing-subscriber` at startup (pretty format, `info` default); full config deferred to Phase 3

**Milestone:** `cargo build` succeeds; `saurron --help` shows all flags; config loads from TOML, env vars, CLI with correct precedence.

---

## Phase 2 — Docker client & container enumeration

**Work:**

- `bollard` integration: Unix socket default (`/var/run/docker.sock`), TLS support (`--tlsverify`, `--tls-ca-cert`, `--tls-cert`, `--tls-key`), configurable API version (`--api-version`)
- Container listing filtered by state (running/stopped/restarting) per flags
- Selection logic: opt-out default, `--label-enable` opt-in, `--disable-containers`, per-container `saurron.*` label parsing

**Milestone:** Binary enumerates containers on live Docker daemon, applies all inclusion/exclusion rules, prints structured list.

---

## Phase 3 — Structured logging & audit trail

**Work:**

- `tracing-subscriber` with `json`/`logfmt`/`pretty`/`auto` formats + level filtering
- Append-only audit log file

**Milestone:** JSON logs flow to stdout; audit events appear in configured file.

---

## Phase 4 — Registry client & freshness detection

**Work:**

- Registry HTTP API v2 client (manifest HEAD requests, `User-Agent` header, `--head-warn-strategy`)
- Digest-pinned image detection → skip with structured warning
- Non-SemVer digest comparison
- SemVer tag enumeration + highest-version selection (pre-release opt-in label)
- Unit tests for digest comparison, SemVer ranking, pre-release filtering; `proptest` for SemVer ranking + image reference parsing

**Milestone:** Monitor-only mode (`--monitor-only`) detects stale containers, logs results, exits cleanly. Notifications deferred to Phase 8.

---

## Phase 5 — Update engine (happy path)

**Work:**

- Pull new image via bollard
- Stop container (configurable stop signal + graceful timeout + SIGKILL)
- Recreate container preserving full run config (env, volumes, networks, labels, ports)
- Dependency ordering (Docker `--link`, `network_mode: container:`, `saurron.depends-on`)
- `--cleanup` post-success image removal
- `--revive-stopped` and `--no-pull` modes

**Milestone:** Single container updated end-to-end on real Docker daemon; config preserved across stop/start; dependency order respected.

---

## Phase 6 — Rollback manager

**Work:**

- Startup monitoring: non-zero exit, healthcheck failure, startup timeout
- On failure: stop new container, restore old image tag, start previous container
- Audit log entries for success + rollback events

**Milestone:** Bad image triggers automatic rollback; rollback event in audit log.

---

## Phase 7 — Scheduler & HTTP API

**Work:**

- Polling scheduler (duration + cron expression inputs)
- `--run-once` mode; validate at startup that `--run-once` not combined with `--interval`/`--schedule`, exit with error if so
- `axum` HTTP server: `POST /v1/update` (including `?image=` and `?container=` scoping params), `GET /v1/health`, `GET /v1/metrics`
- Bearer token auth; `--http-api-metrics-no-auth` option
- Concurrent request handling (§6.7) via `Arc<tokio::sync::Mutex<UpdateState>>`

**Milestone:** Daemon runs with polling + webhook; `/v1/update` triggers immediate cycle; concurrent requests follow skip/merge rules.

---

## Phase 8 — Self-update & graceful shutdown

**Work:**

- Self-update: read own container ID from `$HOSTNAME` (fallback: `/etc/hostname`), query daemon for container name, rename running container to temp name, start replacement under original name
- Self-update failure recovery: if new container doesn't start within timeout, terminate it, log error, rename old container back
- Graceful shutdown on `SIGTERM`/`SIGINT`: finish in-progress update cycle, flush pending notifications, exit cleanly

**Milestone:** Saurron updates its own container image; failure recovery restores old container; `SIGTERM` during active cycle completes current update before exiting.

---

## Phase 9 — Notification system

**Work:**

- Per-cycle event batching
- Webhook target (`reqwest` POST, configurable headers, TLS skip-verify)
- Email target (`lettre`, STARTTLS, configurable SMTP)
- MQTT target (`rumqttc`, configurable QoS + credentials)
- Pushover target (`reqwest` POST to Pushover API, configurable user key + app token)
- `minijinja` template rendering with default + custom templates
- Notification delay

**Milestone:** All four targets deliver correctly formatted batch report after real update cycle; custom template override works.

---

## Phase 10 — Prometheus metrics

**Work:**

- Prometheus metrics (`prometheus` crate) wired to `/v1/metrics`

**Milestone:** `curl /v1/metrics` returns all five metrics with correct values after scan cycle.

---

## Phase 11 — Containerization & integration testing

**Work:**

- `Dockerfile` (multi-stage, minimal final image)
- Integration tests in `tests/` against `docker-compose` fixture (Saurron + dummy updatable container + local registry)
- CI pipeline (build, lint, test, image push with version tag)

**Milestone:** `docker run saurron --run-once` updates container in compose fixture; CI produces tagged image artifact.

---

## Sequencing notes

Phases 1–6 strictly sequential. Phase 7 can start after Phase 5 happy path stable. Phase 8 requires Phase 6 (self-update failure recovery shares rollback concepts). Phases 9 + 10 largely independent once Phase 8 done — can parallelize across Claude sessions.