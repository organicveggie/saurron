# Saurron — Design Document

## 1. Project Name

**Saurron** — ever-watchful eye. Distinct from Go-based Watchtower.

- Crate name: `saurron`
- Container label namespace: `saurron.<label>` (e.g., `saurron.enable`)
- Environment variable prefix: `SAURRON_`

---

## 2. Overview

Rust rewrite of [Watchtower](https://github.com/containrrr/watchtower). Monitors Docker containers on single host, detects newer images, auto-updates with minimal intervention.

Runs as Docker container. Mounts Docker socket to talk to daemon.

---

## 3. Goals

- Auto-detect and apply image updates for running, stopped, restarting containers
- Support semver comparison and digest-based tag-change detection
- Safe update cycle: pull → stop → start → rollback on failure
- Monitor-only mode: detect + notify, no apply
- Run-once mode for external cron
- Rolling restarts: one container at a time
- Self-update own container image
- Flexible scheduling via poll interval and inbound webhook
- Notifications via webhook, email, MQTT; batched per cycle
- Prometheus metrics endpoint
- Structured logs + audit trail of all update/rollback events
- Fully configurable via config file, env vars, CLI flags; Docker secrets support

## 4. Non-Goals (Initial Release)

Deferred to future releases:

- Docker Swarm and Kubernetes support
- Per-registry credential scoping (separate username/password per registry)
- Dependent container restarts
- Notification targets beyond webhook/email/MQTT (Slack, Teams, Gotify, Discord)
- Docker Hub-specific inbound webhook format
- Lifecycle hooks (pre/post-check, pre/post-update shell commands inside containers)
- Scope-based multi-instance support
- Multiple instance detection and deduplication

---

## 5. Architecture Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                            Saurron                               │
│                                                                  │
│  ┌──────────────┐    ┌───────────────────────────────────────┐   │
│  │  Scheduler   │───▶│             Update Engine             │   │
│  │(poll/webhook/│    │ (detect → pull → stop → start / roll) │   │
│  │  run-once)   │    └──────────────────┬────────────────────┘   │
│  └──────────────┘                       │                        │
│                      ┌─────────────────┬┴──────────────────┐     │
│                      ▼                 ▼                   ▼     │
│          ┌──────────────────┐  ┌─────────────┐  ┌────────────┐   │
│          │  Docker Client   │  │  Rollback   │  │  Notifier  │   │
│          │  (Tokio/async)   │  │   Manager   │  │            │   │
│          └────────┬─────────┘  └─────────────┘  └─────┬──────┘   │
│                   │                                   │          │
│  ┌────────────────┴──┐                   ┌────────────┴──────┐   │
│  │  Registry Client  │                   │   Audit Logger    │   │
│  │  (manifest API)   │                   │ (structured log)  │   │
│  └───────────────────┘                   └───────────────────┘   │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │                   Configuration Layer                    │    │
│  │          (config file + env vars + CLI flags)            │    │
│  └──────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘
```

---

## 6. Core Components

### 6.1 Configuration Layer

Precedence order (highest to lowest):

1. **CLI flags**
2. **Environment variables**
3. **Config file** (TOML format)
4. **Built-in defaults**

All sources supported for every option. Config file defaults to `/etc/saurron/config.toml`; override via `--config` / `SAURRON_CONFIG`.

#### Secret File Resolution

If value is path to readable file, transparently replaced with file contents at startup. Enables Docker secrets without embedding sensitive values in env vars or CLI args.

Values supporting substitution:

- `registry_password`
- `http_api.token`
- `notifications.general.template`
- `notifications.webhook.url`
- `notifications.webhook.headers`
- `notifications.email.from`
- `notifications.email.to`
- `notifications.email.server`
- `notifications.email.port`
- `notifications.email.user`
- `notifications.email.password`
- `notifications.mqtt.broker`
- `notifications.mqtt.topic`
- `notifications.mqtt.client_id`
- `notifications.mqtt.username`
- `notifications.mqtt.password`
- `notifications.pushover.token`
- `notifications.pushover.user_key`

### 6.2 Docker Client

- Communicates via Unix socket (`/var/run/docker.sock` default), mounted into container
- Supports TLS-secured daemon connections (`--tlsverify`, `--tls-ca-cert`, `--tls-cert`, `--tls-key`)
- Docker API version configurable (`--api-version`) for older daemon compat
- Async Rust Docker API client on Tokio
- Responsibilities:
  - Enumerate running/stopped/restarting containers + image metadata (configurable scope)
  - Pull updated images
  - Stop and start containers (preserving original config: env, volumes, networks, labels, ports, stop signal)
  - Query container health for rollback decisions
  - Rename containers (required for self-update)
  - Remove old images after successful update (optional)

### 6.3 Registry Client

- Queries registries via Docker Registry HTTP API v2
- Fetches manifests via authenticated HEAD to compare digests without pulling
- If manifest can't determine freshness, container skipped + error logged. Three known cases:
  - Registry returns empty manifest list
  - Manifest list has no entry for container's target architecture
  - Registry returns malformed/unexpected response (treated as registry bug or transient error)
- Sends `User-Agent: saurron/<version>` on all outbound registry requests
- **Authentication** — optional global username/password (`--registry-username` / `--registry-password`). When provided, credentials are sent as HTTP Basic Auth to the registry's token endpoint (from the `WWW-Authenticate: Bearer realm=...` challenge) to obtain a scoped Bearer token. Credentials apply to all registries; per-registry scoping is a future enhancement.
- **HEAD request warning strategy** — configurable:
  - `auto` (default): warn only for registries known to support HEAD reliably (Docker Hub, ghcr.io); suppress for others
  - `always`: always warn on HEAD failure
  - `never`: suppress all HEAD failure warnings

### 6.4 Image Freshness Detection

Strategy depends on how image is referenced:

#### Digest-Pinned Images (e.g., `myapp@sha256:abc123...`)

**Always skipped.** Digest pin = exact content address; no "newer version" concept. Structured warning emitted per skipped container.

#### Semantic Version Tags (e.g., `myapp:1.2.3`, `myapp:v1.2.3`)

Tag recognized as SemVer if matches grammar from [SemVer 2.0.0 spec](https://semver.org/#backusnaur-form-grammar-for-valid-semver-versions), plus optional `v` prefix. Non-matching tags treated as non-SemVer.

- Registry queried for all available tags
- Each tag tested against SemVer grammar (with optional `v`); non-matching ignored
- Highest version greater than current selected
- Pre-release versions (e.g., `1.2.3-beta`) ignored by default; set `saurron.semver-pre-release=true` label to include them

#### Non-SemVer Tags (e.g., `myapp:latest`, `myapp:stable`, `myapp:20240101`)

Fully supported via digest comparison:

- **Default**: manifest digest of running image compared to digest currently resolved by registry for same tag; differ = stale = update triggered
- **Per-container override**: label can opt container out of digest comparison entirely (see Section 7)

### 6.5 Update Engine

Orchestrates full lifecycle of a single update cycle.

#### Standard Mode

Stale containers updated one at a time in reverse dependency order (leaves first). Dependencies detected from Docker `--link`, `network_mode: container:`, and `saurron.depends-on` label (see Section 7). Each container fully updated before next begins, limiting blast radius of failed update:

```
1. Scan all matched containers for stale images
2. Sort stale containers in reverse dependency order
3. For each stale container:
   a. Pull new image
   b. Stop old container (send configured stop signal, wait for graceful exit, SIGKILL on timeout)
   c. Start new container with original configuration
   d. Monitor container for successful startup (see §6.6)
   e. On success: if `--cleanup` is enabled, remove the old image (the old image is retained until this point to support rollback)
   f. On failure: invoke Rollback Manager (see §6.6)
4. Emit session report
```

#### Monitor-Only Mode (`--monitor-only`)

Detects stale containers, sends notifications, no pull or restart. Available globally and per-container via label (see Section 7).

#### Revive Stopped Mode (`--revive-stopped`)

Default: stopped containers untouched — no pull, no recreate, no start. When enabled, stopped containers treated like running: pull new image, recreate, start. Defaults off to prevent auto-restarting intentionally stopped containers.

#### Run-Once Mode (`--run-once`)

Single cycle — scan, update, notify — then exit. For external schedulers (e.g., system cron, Kubernetes CronJob).

#### Self-Update

When own container image is stale, uses special handling. Reads container ID from `$HOSTNAME` (fallback: `/etc/hostname`), queries daemon for container name, renames running container to temp name, starts replacement using original name.

If new container doesn't start within configurable timeout: terminate it, log error, rename old container back. No further recovery; operator intervention required.

> **Note:** Self-update has brief window where Saurron is not running — between stopping old container and new one reaching healthy state. No monitoring or updates during this window. Expected and unavoidable.

#### Graceful Shutdown

On `SIGTERM` or `SIGINT`: finish in-progress update cycle, flush pending notifications, exit cleanly.

All steps logged to audit trail.

### 6.6 Rollback Manager

If new container doesn't reach healthy running state:

1. Stop new container
2. Restore previous image tag (old image always retained until new container confirmed healthy, even when `--cleanup` enabled)
3. Start original container from previous image
4. Emit rollback event to audit log and notifier

**Failure conditions** — user-configurable; any combination can be enabled:

| Condition             | Description                                                      | Default |
| --------------------- | ---------------------------------------------------------------- | ------- |
| `non-zero-exit`       | Container exits with non-zero code immediately after start       | Enabled |
| `healthcheck-failure` | Docker healthcheck reports unhealthy within configurable timeout | Enabled |
| `startup-timeout`     | Container doesn't reach `running` state within N seconds         | Enabled |

Timeout for `healthcheck-failure` and `startup-timeout` configurable (default: 30 seconds).

### 6.7 Scheduler

Polling and inbound webhook server are independent; both can be active simultaneously.

#### Polling

- Runs update check on configurable interval (cron expression or simple duration, e.g., `5m`, `1h`)
- Default interval: 24 hours

#### Inbound Webhook

- Lightweight HTTP server on configurable port (default: `8080`)
- `POST /v1/update` triggers immediate update check
- Optionally scoped to specific container or image via query parameter
- Optional shared secret token (Bearer auth)
- Concurrency rules when request arrives during in-progress update (warning logged in all cases):
  - **Targeted request, already being updated**: ignored, success returned immediately
  - **Targeted request, not being updated**: proceeds normally
  - **Full-scan request, full scan already in progress**: ignored, success returned immediately
  - **Full-scan request, one or more targeted updates in progress**: full scan proceeds but skips already-updating containers/images
- In-progress state tracked in `Arc<tokio::sync::Mutex<UpdateState>>` (set of in-progress container names + full-scan flag). `tokio::sync::Mutex` used instead of `std::sync::Mutex` so guard can be held safely across `.await` points.

#### Run-Once

- Single update cycle then exit (see §6.5)
- Mutually exclusive with polling and webhook modes

### 6.8 Notification System

#### Batching

Batched per update cycle. All events from single scan accumulated and delivered together on cycle completion, not one per container. Configurable delay (default: 0) between cycle completion and dispatch — useful for rate-limiting or debouncing.

#### Events

Notifications sent on:

- One or more containers successfully updated
- One or more containers detected stale in monitor-only mode (global or per-container)
- Rollback triggered
- Update check errors

Suppressed if cycle produced no updates, no stale detections, no failures.

#### Supported Targets

| Target   | Transport | Notes                                                         |
| -------- | --------- | ------------------------------------------------------------- |
| Webhook  | HTTP POST | Generic JSON payload; configurable URL and headers            |
| Email    | SMTP      | STARTTLS by default; configurable server, port, credentials   |
| MQTT     | TCP/TLS   | Configurable broker URL, topic, QoS, and credentials          |
| Pushover | TCP       | Real-time notifications on Android, iPhone, iPad, and Desktop |

Multiple targets can be active simultaneously.

#### Notification Payload

```json
{
  "timestamp": "2026-04-13T10:00:00Z",
  "hostname": "my-docker-host",
  "summary": {
    "scanned": 5,
    "updated": 2,
    "stale_detected": 1,
    "failed": 0
  },
  "containers": [
    {
      "name": "myapp",
      "old_image": "myorg/myapp:1.2.3@sha256:abc123...",
      "new_image": "myorg/myapp:1.2.4@sha256:def456...",
      "outcome": "updated"
    }
  ]
}
```

`outcome` values:

| Value            | Description                                                         |
| ---------------- | ------------------------------------------------------------------- |
| `updated`        | Image pulled, container restarted successfully                      |
| `stale_detected` | Update detected in monitor-only mode; no restart                    |
| `rolled_back`    | Container started but failed health checks; previous image restored |
| `failed`         | Update attempted but failed; rollback not possible                  |

`old_image` and `new_image` are fully-qualified in `name:tag@sha256:digest` form. For `stale_detected`, `new_image` = digest currently resolved by registry; no pull occurred. For `failed` where failure happened before pull completed, `new_image` may be `null`.

#### Custom Templates

Uses [MiniJinja](https://github.com/mitsuhiko/minijinja) (Jinja2-compatible). Template context exposes same fields as webhook JSON payload: `timestamp`, `hostname`, `summary` (`scanned`, `updated`, `stale_detected`, `failed`), `containers` (list with `name`, `old_image`, `new_image`, `outcome`).

Built-in default template:

```jinja
Saurron update report — {{ timestamp }}
Host: {{ hostname }}

Summary: {{ summary.scanned }} scanned, {{ summary.updated }} updated, {{ summary.stale_detected }} stale detected, {{ summary.failed }} failed

Containers:
{% for container in containers %}  {{ container.name }}
    old: {{ container.old_image }}
    new: {{ container.new_image | default("n/a") }}
    outcome: {{ container.outcome }}
{% endfor %}
```

Override via `--notification-template` / `SAURRON_NOTIFICATION_TEMPLATE` / `notifications.general.template`.

### 6.9 Structured Logging & Audit Trail

- All logs structured; formats: `json`, `logfmt`, `pretty`, `auto` (pretty on TTY, logfmt otherwise)
- Log levels: `trace`, `debug`, `info`, `warn`, `error`; shorthand `--debug` and `--trace` flags
- **Audit trail** records every update and rollback with full context:
  - Timestamp
  - Container name and ID
  - Previous image digest and tag
  - New image digest and tag
  - Outcome (success / rolled back / failed)
  - Failure reason (if applicable)
- Audit entries written to dedicated append-only log file (path configurable) in addition to main log stream

### 6.10 Build Version Metadata

- Version string injected at build time (default: `v0.0.0-unknown` for local builds)
- Included in startup log output
- Drives `User-Agent` header sent to registries (see §6.3)

---

## 7. Container Selection

### Default Behaviour

Only running containers are candidates by default. Stopped and restarting containers included via flags.

### Inclusion Flags

| Flag                   | Description                                                     |
| ---------------------- | --------------------------------------------------------------- |
| `--revive-stopped`     | Treat containers in `created` or `exited` state same as running |
| `--include-restarting` | Include containers in `restarting` state                        |

### Opt-Out (Default Mode)

Exclude by setting opt-out label:

```
saurron.enable=false
```

Also exclude by name via `--disable-containers` (comma-separated).

### Opt-In Mode (`--label-enable`)

Only containers with `saurron.enable=true` included. All others ignored.

### Per-Container Configuration Labels

| Label                         | Values                          | Description                                                                                                           |
| ----------------------------- | ------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `saurron.enable`              | `true` / `false`                | Include or exclude (or mark opt-in when `--label-enable` active)                                                      |
| `saurron.monitor-only`        | `true` / `false`                | Detect + notify; do not restart                                                                                       |
| `saurron.no-pull`             | `true` / `false`                | Restart from cached image without pulling                                                                             |
| `saurron.stop-signal`         | signal name (e.g., `SIGHUP`)    | Override stop signal                                                                                                  |
| `saurron.depends-on`          | comma-separated container names | Explicit dependencies beyond Docker `--link` and `network_mode: container:`                                           |
| `saurron.non-semver-strategy` | `skip` / `digest`               | Override non-semver tag strategy. Default `digest`; `skip` disables update checks for this container's non-semver tag |
| `saurron.semver-pre-release`  | `true` / `false`                | Include pre-release versions when selecting latest semver tag. No effect on non-semver. Default: `false`              |
| `saurron.stop-timeout`        | duration (e.g., `30s`)          | Override graceful stop timeout                                                                                        |

### Label Precedence

Default: per-container labels for `monitor-only` and `no-pull` take precedence over global flags. Set `--global-takes-precedence` to invert.

---

## 8. HTTP API

Server starts on configurable port (default: `8080`) when any API feature enabled. All endpoints except `GET /v1/health` require Bearer token auth (`Authorization: Bearer <token>`). If token not configured, process exits with error at startup.

`GET /v1/metrics` can be made unauthenticated via `--http-api-metrics-no-auth` — useful when Prometheus scrapes from trusted network without bearer token support.

### `POST /v1/update`

Trigger immediate update check.

**Query parameters (optional):**

- `?image=myorg/myapp` — restrict to containers using this image (comma-separated)
- `?container=mycontainer` — restrict to specific container by name (comma-separated)

**Response:**

```json
{
  "status": "triggered",
  "timestamp": "2026-04-13T10:00:00Z"
}
```

`status` values:

- `"triggered"` — update cycle started
- `"skipped"` — dropped; target (or full scan) already in progress
- `"merged"` — full-scan accepted but some containers already updating were excluded

See §6.7 for concurrency behaviour. Response always `200 OK`.

### `GET /v1/metrics`

Prometheus metrics in standard text exposition format.

Tracked metrics:

| Metric                       | Type    | Description                                    |
| ---------------------------- | ------- | ---------------------------------------------- |
| `saurron_scans_total`        | Counter | Total update cycles run                        |
| `saurron_scans_skipped`      | Counter | Cycles skipped due to concurrent update        |
| `saurron_containers_scanned` | Gauge   | Containers checked in last cycle               |
| `saurron_containers_updated` | Gauge   | Containers updated in last cycle               |
| `saurron_containers_failed`  | Gauge   | Containers that failed to update in last cycle |

### `GET /v1/health`

Returns `200 OK` when service running. Suitable as Docker healthcheck. Unauthenticated; no Bearer token required.

---

## 9. Testing Strategy

### Unit tests

Live in `#[cfg(test)]` modules alongside code, per standard Rust convention. Every module with non-trivial logic ships unit tests. Priority targets:

- Config precedence and secret file resolution
- SemVer tag ranking and pre-release filtering
- Digest comparison and freshness detection
- Dependency ordering (reverse topological sort)
- Rollback condition evaluation

### Property-based tests

Use `proptest`; live in same `#[cfg(test)]` module. Used where input space is large or edge cases hard to enumerate:

- SemVer tag ranking (arbitrary tag strings, arbitrary version sets)
- Digest and image reference parsing

### Integration tests

End-to-end in top-level `tests/` directory; run against real Docker daemon via `docker-compose` fixture (Saurron + dummy updatable container + local registry). Cover full update cycles, rollback, and notification delivery.

---

## 10. Technology Stack

| Concern                       | Choice                           | Rationale                                                                                          |
| ----------------------------- | -------------------------------- | -------------------------------------------------------------------------------------------------- |
| Language                      | Rust (stable)                    | Memory safety, performance, strong async ecosystem                                                 |
| Async runtime                 | Tokio                            | De-facto standard; excellent ecosystem support                                                     |
| Config                        | `config-rs` + `clap`             | Layered config (file + env + CLI)                                                                  |
| Docker API                    | `bollard`                        | Leading async Rust Docker client; actively maintained, hyper 1.x, full API coverage                |
| Email                         | `lettre`                         | Pure Rust SMTP client with STARTTLS support                                                        |
| Error handling                | `thiserror` + `anyhow`           | `thiserror` for domain error types in core modules; `anyhow` in `main` and top-level orchestration |
| HTTP client (registry)        | `reqwest`                        | Tokio-native, widely used                                                                          |
| HTTP server (webhook + API)   | `axum`                           | Tokio-native, ergonomic, well-maintained                                                           |
| Logging                       | `tracing` + `tracing-subscriber` | Structured, async-aware logging                                                                    |
| MQTT                          | `rumqttc`                        | Async Rust MQTT client                                                                             |
| Notification templates        | `minijinja`                      | Runtime Jinja2-style templating; single required dependency (`serde`)                              |
| Prometheus metrics            | `prometheus`                     | Standard Rust Prometheus client                                                                    |
| Serialization                 | `serde` + `serde_json`           | Standard Rust serialization                                                                        |
| SemVer parsing                | `semver`                         | Official SemVer crate from the Cargo ecosystem                                                     |
| Unit testing (property-based) | `proptest`                       | Property-based tests for SemVer ranking, digest parsing, and config merging                        |

---

## 11. Configuration Reference

> All environment variables use the prefix `SAURRON_`.

### 11.1 General

| Purpose                                                                | CLI Flag                | Environment Variable | TOML Key           |
| ---------------------------------------------------------------------- | ----------------------- | -------------------- | ------------------ |
| Path to TOML config file                                               | `--config <path>`       | `SAURRON_CONFIG`     | _(not applicable)_ |
| Log level (`trace`, `debug`, `info`, `warn`, `error`). Default: `info` | `--log-level <level>`   | `SAURRON_LOG_LEVEL`  | `log_level`        |
| Log format (`auto`, `json`, `logfmt`, `pretty`). Default: `auto`       | `--log-format <format>` | `SAURRON_LOG_FORMAT` | `log_format`       |
| Shorthand for `--log-level debug`                                      | `--debug`               | —                    | —                  |
| Shorthand for `--log-level trace`                                      | `--trace`               | —                    | —                  |
| Path to append-only audit log file                                     | `--audit-log <path>`    | `SAURRON_AUDIT_LOG`  | `audit_log`        |

### 11.2 Docker Connection

| Purpose                                                                  | CLI Flag                  | Environment Variable | TOML Key             |
| ------------------------------------------------------------------------ | ------------------------- | -------------------- | -------------------- |
| Docker daemon socket or host URL. Default: `unix:///var/run/docker.sock` | `--host <uri>`            | `DOCKER_HOST`        | `docker.host`        |
| Enable TLS for Docker daemon connection                                  | `--tlsverify`             | `DOCKER_TLS_VERIFY`  | `docker.tls_verify`  |
| Path to TLS CA certificate                                               | `--tls-ca-cert <path>`    | `DOCKER_CERT_PATH`   | `docker.tls_ca_cert` |
| Path to TLS client certificate                                           | `--tls-cert <path>`       | —                    | `docker.tls_cert`    |
| Path to TLS client key                                                   | `--tls-key <path>`        | —                    | `docker.tls_key`     |
| Docker API version to negotiate. Default: auto-negotiate                 | `--api-version <version>` | `DOCKER_API_VERSION` | `docker.api_version` |

### 11.3 Scheduling

| Purpose                                                                                                                          | CLI Flag                | Environment Variable    | TOML Key        |
| -------------------------------------------------------------------------------------------------------------------------------- | ----------------------- | ----------------------- | --------------- |
| Poll interval as duration (e.g., `5m`, `1h`). Converted to cron internally. Mutually exclusive with `--schedule`. Default: `24h` | `--interval <duration>` | `SAURRON_POLL_INTERVAL` | `poll_interval` |
| Poll schedule as cron expression (e.g., `0 4 * * *`). Mutually exclusive with `--interval`                                       | `--schedule <cron>`     | `SAURRON_SCHEDULE`      | `schedule`      |
| Single update cycle then exit. Mutually exclusive with `--interval` and `--schedule`                                             | `--run-once`            | `SAURRON_RUN_ONCE`      | `run_once`      |

### 11.4 Container Selection

| Purpose                                                                                                             | CLI Flag                       | Environment Variable              | TOML Key                  |
| ------------------------------------------------------------------------------------------------------------------- | ------------------------------ | --------------------------------- | ------------------------- |
| Opt-in mode: only update containers with `saurron.enable=true`                                                      | `--label-enable`               | `SAURRON_LABEL_ENABLE`            | `label_enable`            |
| Comma-separated container names to always exclude                                                                   | `--disable-containers <names>` | `SAURRON_DISABLE_CONTAINERS`      | `disable_containers`      |
| Include containers in `restarting` state                                                                            | `--include-restarting`         | `SAURRON_INCLUDE_RESTARTING`      | `include_restarting`      |
| Global flags take precedence over per-container labels for `monitor-only` and `no-pull` (default: label precedence) | `--global-takes-precedence`    | `SAURRON_GLOBAL_TAKES_PRECEDENCE` | `global_takes_precedence` |

### 11.5 Update Strategy

| Purpose                                                    | CLI Flag                    | Environment Variable     | TOML Key         |
| ---------------------------------------------------------- | --------------------------- | ------------------------ | ---------------- |
| Detect + notify; no pull or restart                        | `--monitor-only`            | `SAURRON_MONITOR_ONLY`   | `monitor_only`   |
| Restart using cached image without pulling                 | `--no-pull`                 | `SAURRON_NO_PULL`        | `no_pull`        |
| Remove old images after successful update                  | `--cleanup`                 | `SAURRON_CLEANUP`        | `cleanup`        |
| Start stopped containers after image update                | `--revive-stopped`          | `SAURRON_REVIVE_STOPPED` | `revive_stopped` |
| Wait time for graceful stop before SIGKILL. Default: `10s` | `--stop-timeout <duration>` | `SAURRON_STOP_TIMEOUT`   | `stop_timeout`   |

### 11.6 Rollback

| Purpose                                                                                   | CLI Flag                                                     | Environment Variable              | TOML Key                   |
| ----------------------------------------------------------------------------------------- | ------------------------------------------------------------ | --------------------------------- | -------------------------- |
| Rollback if new container exits non-zero. Default: enabled                                | `--rollback-on-exit-code` / `--no-rollback-on-exit-code`     | `SAURRON_ROLLBACK_ON_EXIT_CODE`   | `rollback.on_exit_code`    |
| Rollback if Docker healthcheck reports unhealthy within startup timeout. Default: enabled | `--rollback-on-healthcheck` / `--no-rollback-on-healthcheck` | `SAURRON_ROLLBACK_ON_HEALTHCHECK` | `rollback.on_healthcheck`  |
| Rollback if container doesn't reach `running` within startup timeout. Default: enabled    | `--rollback-on-timeout` / `--no-rollback-on-timeout`         | `SAURRON_ROLLBACK_ON_TIMEOUT`     | `rollback.on_timeout`      |
| Wait time before triggering rollback. Default: `30s`                                      | `--startup-timeout <duration>`                               | `SAURRON_STARTUP_TIMEOUT`         | `rollback.startup_timeout` |

### 11.7 Registry

| Purpose                                                                                                                | CLI Flag                            | Environment Variable           | TOML Key              |
| ---------------------------------------------------------------------------------------------------------------------- | ----------------------------------- | ------------------------------ | --------------------- |
| Warning behaviour for failed HEAD requests: `auto` (default — warn only for Docker Hub and ghcr.io), `always`, `never` | `--head-warn-strategy <strategy>`   | `SAURRON_HEAD_WARN_STRATEGY`   | `head_warn_strategy`  |
| Username for registry authentication (applied to all registries)                                                       | `--registry-username <username>`    | `SAURRON_REGISTRY_USERNAME`    | `registry_username`   |
| Password for registry authentication; supports Docker secret file path                                                 | `--registry-password <password>`    | `SAURRON_REGISTRY_PASSWORD`    | `registry_password`   |

### 11.8 HTTP API

| Purpose                                                        | CLI Flag                     | Environment Variable               | TOML Key                   |
| -------------------------------------------------------------- | ---------------------------- | ---------------------------------- | -------------------------- |
| Enable `POST /v1/update`                                       | `--http-api-update`          | `SAURRON_HTTP_API_UPDATE`          | `http_api.update`          |
| Enable `GET /v1/metrics`                                       | `--http-api-metrics`         | `SAURRON_HTTP_API_METRICS`         | `http_api.metrics`         |
| Bearer token for all API requests                              | `--http-api-token <token>`   | `SAURRON_HTTP_API_TOKEN`           | `http_api.token`           |
| HTTP API server port. Default: `8080`                          | `--http-api-port <port>`     | `SAURRON_HTTP_API_PORT`            | `http_api.port`            |
| Serve `GET /v1/metrics` without Bearer token. Default: `false` | `--http-api-metrics-no-auth` | `SAURRON_HTTP_API_METRICS_NO_AUTH` | `http_api.metrics_no_auth` |

### 11.9 Notifications — General

| Purpose                                                                               | CLI Flag                             | Environment Variable            | TOML Key                         |
| ------------------------------------------------------------------------------------- | ------------------------------------ | ------------------------------- | -------------------------------- |
| Delay between cycle completion and notification dispatch (e.g., `30s`). Default: `0s` | `--notification-delay <duration>`    | `SAURRON_NOTIFICATION_DELAY`    | `notifications.general.delay`    |
| Custom notification template string; uses built-in default when omitted               | `--notification-template <template>` | `SAURRON_NOTIFICATION_TEMPLATE` | `notifications.general.template` |

### 11.10 Notifications — Webhook

| Purpose                                                                                  | CLI Flag                      | Environment Variable              | TOML Key                                |
| ---------------------------------------------------------------------------------------- | ----------------------------- | --------------------------------- | --------------------------------------- |
| URL to POST notification payloads to                                                     | `--webhook-url <url>`         | `SAURRON_WEBHOOK_URL`             | `notifications.webhook.url`             |
| Additional HTTP headers as comma-separated `Key:Value` pairs                             | `--webhook-headers <headers>` | `SAURRON_WEBHOOK_HEADERS`         | `notifications.webhook.headers`         |
| Skip TLS cert verification. Default: `false` — invalid cert logs error and skips webhook | `--webhook-tls-skip-verify`   | `SAURRON_WEBHOOK_TLS_SKIP_VERIFY` | `notifications.webhook.tls_skip_verify` |

### 11.11 Notifications — Email

| Purpose                                | CLI Flag                                   | Environment Variable                         | TOML Key                              |
| -------------------------------------- | ------------------------------------------ | -------------------------------------------- | ------------------------------------- |
| Sender address                         | `--notification-email-from <address>`      | `SAURRON_NOTIFICATION_EMAIL_FROM`            | `notifications.email.from`            |
| Recipient address(es), comma-separated | `--notification-email-to <addresses>`      | `SAURRON_NOTIFICATION_EMAIL_TO`              | `notifications.email.to`              |
| SMTP server hostname                   | `--notification-email-server <host>`       | `SAURRON_NOTIFICATION_EMAIL_SERVER`          | `notifications.email.server`          |
| SMTP server port. Default: `587`       | `--notification-email-port <port>`         | `SAURRON_NOTIFICATION_EMAIL_PORT`            | `notifications.email.port`            |
| SMTP auth username                     | `--notification-email-user <user>`         | `SAURRON_NOTIFICATION_EMAIL_USER`            | `notifications.email.user`            |
| SMTP auth password                     | `--notification-email-password <password>` | `SAURRON_NOTIFICATION_EMAIL_PASSWORD`        | `notifications.email.password`        |
| Skip TLS cert verification for SMTP    | `--notification-email-tls-skip-verify`     | `SAURRON_NOTIFICATION_EMAIL_TLS_SKIP_VERIFY` | `notifications.email.tls_skip_verify` |

### 11.12 Notifications — MQTT

| Purpose                                                                                    | CLI Flag                                  | Environment Variable                  | TOML Key                       |
| ------------------------------------------------------------------------------------------ | ----------------------------------------- | ------------------------------------- | ------------------------------ |
| MQTT broker URL (e.g., `tcp://broker.example.com:1883` or `ssl://broker.example.com:8883`) | `--notification-mqtt-broker <url>`        | `SAURRON_NOTIFICATION_MQTT_BROKER`    | `notifications.mqtt.broker`    |
| MQTT topic for notifications                                                               | `--notification-mqtt-topic <topic>`       | `SAURRON_NOTIFICATION_MQTT_TOPIC`     | `notifications.mqtt.topic`     |
| MQTT QoS: `0` (at most once), `1` (at least once), `2` (exactly once). Default: `0`        | `--notification-mqtt-qos <level>`         | `SAURRON_NOTIFICATION_MQTT_QOS`       | `notifications.mqtt.qos`       |
| MQTT client ID; auto-generated if omitted                                                  | `--notification-mqtt-client-id <id>`      | `SAURRON_NOTIFICATION_MQTT_CLIENT_ID` | `notifications.mqtt.client_id` |
| MQTT broker auth username                                                                  | `--notification-mqtt-username <user>`     | `SAURRON_NOTIFICATION_MQTT_USERNAME`  | `notifications.mqtt.username`  |
| MQTT broker auth password                                                                  | `--notification-mqtt-password <password>` | `SAURRON_NOTIFICATION_MQTT_PASSWORD`  | `notifications.mqtt.password`  |

### 11.13 Notifications — Pushover

| Purpose                        | CLI Flag                                 | Environment Variable                     | TOML Key                          |
| ------------------------------ | ---------------------------------------- | ---------------------------------------- | --------------------------------- |
| Pushover application API token | `--notification-pushover-token <token>`  | `SAURRON_NOTIFICATION_PUSHOVER_TOKEN`    | `notifications.pushover.token`    |
| Pushover user or group key     | `--notification-pushover-user-key <key>` | `SAURRON_NOTIFICATION_PUSHOVER_USER_KEY` | `notifications.pushover.user_key` |

---

## 12. Future Enhancements

Deferred from initial release:

| Feature                            | Notes                                                                                                                       |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Docker Swarm support               | Multi-host orchestration                                                                                                    |
| Kubernetes support                 | Controller/operator model                                                                                                   |
| Per-registry credential scoping    | Separate username/password per registry; Docker config file credential source                                               |
| Dependent container restarts       | Restart containers sharing networks or volumes with updated container                                                       |
| Slack notifications                |                                                                                                                             |
| Microsoft Teams notifications      |                                                                                                                             |
| Gotify notifications               |                                                                                                                             |
| Discord notifications              |                                                                                                                             |
| Docker Hub inbound webhook format  | Parse Docker Hub-specific webhook payloads                                                                                  |
| Web UI                             | Dashboard for update history and manual triggers                                                                            |
| Lifecycle hooks                    | Pre/post-check and pre/post-update shell commands inside containers; `EX_TEMPFAIL` exit code to signal skip-without-failure |
| Notification template preview      | Validate custom templates against synthetic data without real update cycle                                                  |
| Scope-based multi-instance support | Multiple instances on same host managing non-overlapping container sets via scope label                                     |
| Multiple instance detection        | Detect duplicate instances sharing same scope; stop all but most recently created                                           |

---

_Document status: Draft_
