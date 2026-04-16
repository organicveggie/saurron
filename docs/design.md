# Saurron — Design Document

## 1. Project Name

The project is named **Saurron**. The name evokes an ever-watchful eye — a deliberate nod to the all-seeing nature of the tool — while remaining distinct from the existing Go-based Watchtower project.

- Crate name: `saurron`
- Container label namespace: `saurron.<label>` (e.g., `saurron.enable`)
- Environment variable prefix: `SAURRON_`

---

## 2. Overview

This project is a rewrite of [Watchtower](https://github.com/containrrr/watchtower) in Rust. It monitors Docker containers on a single host, detects when a newer image is available, and automatically updates containers with minimal manual intervention.

The software runs as a Docker container itself, mounting the Docker socket to communicate with the Docker daemon.

---

## 3. Goals

- Automatically detect and apply image updates for running, stopped, and restarting Docker containers
- Support both semantic version comparison and digest-based tag-change detection
- Provide a safe update cycle: pull new image → stop old container → start new container → roll back on failure
- Support monitor-only mode: detect updates and notify without applying them
- Support run-once mode for external cron job integration
- Support rolling restarts: update one container at a time rather than all at once
- Automatically update the tool's own container image
- Support flexible scheduling via polling interval and inbound webhook triggers
- Deliver update notifications via webhooks, email, and MQTT; batch notifications per update cycle
- Expose a Prometheus metrics endpoint for operational observability
- Emit structured logs and maintain an audit trail of all update and rollback events
- Be fully configurable via config file, environment variables, and CLI flags, with Docker secrets support

## 4. Non-Goals (Initial Release)

The following are explicitly deferred to future releases:

- Docker Swarm and Kubernetes support
- Private registry authentication
- Restarting containers that depend on an updated container
- Notification targets beyond webhooks, email, and MQTT (Slack, Teams, Gotify, Discord)
- Docker Hub-specific inbound webhook format
- Lifecycle hooks (pre/post-check, pre/post-update shell commands inside containers)
- Scope-based multi-instance support (multiple tool instances with non-overlapping container scopes)
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

Configuration is resolved in the following precedence order (highest to lowest):

1. **CLI flags**
2. **Environment variables**
3. **Config file** (TOML format)
4. **Built-in defaults**

All three sources are supported for every option. The config file path defaults to `/etc/saurron/config.toml` and can be overridden via `--config` / `SAURRON_CONFIG`.

#### Secret File Resolution

The following configuration values support secret file resolution: if the value is a path to a readable file, it is transparently replaced with that file's contents at startup. This enables Docker secrets and mounted credential files without embedding sensitive values in environment variables or command-line arguments.

Values that support this substitution:

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

### 6.2 Docker Client

- Communicates with the Docker daemon via the Unix socket (`/var/run/docker.sock` by default), mounted into the container
- Supports TLS-secured Docker daemon connections (`--tlsverify`, `--tls-ca-cert`, `--tls-cert`, `--tls-key`)
- Docker API version is configurable (`--api-version`) for compatibility with older daemons
- Uses an async Rust Docker API client built on Tokio
- Responsibilities:
  - Enumerate running, stopped, and restarting containers and their image metadata (configurable scope)
  - Pull updated images
  - Stop and start containers (preserving original run configuration: env, volumes, networks, labels, ports, stop signal)
  - Query container health status for rollback decisions
  - Rename containers (required for self-update)
  - Remove old images after successful updates (optional)

### 6.3 Registry Client

- Queries container registries using the Docker Registry HTTP API v2
- Fetches image manifests via authenticated HEAD requests to compare digests without pulling
- If the manifest response cannot be used to determine freshness, the container is skipped and an error is logged. This covers three known cases:
  - The registry returns an empty manifest list
  - The manifest list does not include an entry for the container's target architecture
  - The registry returns a malformed or unexpected response (treated as a registry bug or transient error)
- Only public registries are supported in the initial release
- Sends a `User-Agent` header of the form `saurron/<version>` with all outbound registry requests, making the tool identifiable in registry access logs
- **HEAD request warning strategy**: Configurable behaviour for failed HEAD requests:
  - `auto` (default): warn only for registries known to support HEAD reliably (Docker Hub, ghcr.io); suppress warnings for others where HEAD may simply be unimplemented
  - `always`: always emit a warning on HEAD failure
  - `never`: suppress all HEAD failure warnings

### 6.4 Image Freshness Detection

The update check strategy depends on how the image is referenced:

#### Digest-Pinned Images (e.g., `myapp@sha256:abc123...`)

Images referenced by a `sha256:` digest rather than a tag are **always skipped**. A digest pin is an exact content address — there is no "newer version" concept for a pinned digest. A structured warning is emitted for each skipped container.

#### Semantic Version Tags (e.g., `myapp:1.2.3`, `myapp:v1.2.3`)

A tag is recognised as a SemVer tag if it matches the grammar defined in the [SemVer 2.0.0 specification](https://semver.org/#backusnaur-form-grammar-for-valid-semver-versions), with one extension: an optional `v` prefix is accepted (i.e., `v<valid semver>` is treated equivalently to `<valid semver>`). Tags that do not match are treated as non-SemVer (see below).

- The registry is queried for all available tags for the image
- Each tag is tested against the SemVer grammar (with optional `v` prefix); non-matching tags are ignored
- The highest version greater than the currently running version is selected
- Pre-release versions (e.g., `1.2.3-beta`) are ignored by default; set the `saurron.semver-pre-release=true` label on a container to include pre-release versions in the update check for that container

#### Non-SemVer Tags (e.g., `myapp:latest`, `myapp:stable`, `myapp:20240101`)

Non-semver tags (including `latest`) are fully supported via digest comparison:

- **Default behaviour**: The manifest digest of the running image is compared to the digest currently resolved by the registry for the same tag; if the digests differ, the image is considered stale and an update is triggered
- **Per-container override**: A container label can opt the container out of digest comparison, skipping the update check entirely (see Section 7 — Container Selection)

### 6.5 Update Engine

The update engine orchestrates the full lifecycle of a single update cycle.

#### Standard Mode

Stale containers are updated one at a time in reverse dependency order (leaves first). Dependency relationships are detected from Docker `--link`, `network_mode: container:`, and the `saurron.depends-on` label (see Section 7). Each container is fully updated before the next one begins, limiting the blast radius of a failed update:

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

The engine detects stale containers and sends notifications but does not pull new images or restart any containers. Available as a global flag and as a per-container label (see Section 7).

#### Revive Stopped Mode (`--revive-stopped`)

By default, stopped containers are not touched at all — Saurron will not pull a new image, will not recreate the container, and will not start it. When `--revive-stopped` is enabled, Saurron treats stopped containers like running ones: it pulls the new image, recreates the container from it, and starts it. This flag defaults to off to ensure Saurron never automatically restarts a container that was intentionally stopped.

#### Run-Once Mode (`--run-once`)

Performs a single update cycle — scan, update, notify — then exits. Suitable for invocation by an external scheduler (e.g., system cron, Kubernetes CronJob).

#### Self-Update

When the tool's own container image is stale, it updates itself using special handling. Before starting the replacement container, Saurron determines its own container name by reading its container ID from `$HOSTNAME` (or `/etc/hostname` as a fallback) and querying the Docker daemon for the container name associated with that ID. The running container is then renamed to a temporary name, and the replacement container is started using the original name.

> **Note:** Self-update introduces a brief window during which Saurron is not running — between stopping the old container and the new one reaching a healthy state. No monitoring or updates occur during this window. This is expected and unavoidable.

#### Graceful Shutdown

On receipt of `SIGTERM` or `SIGINT`, the tool finishes any in-progress update cycle, flushes pending notifications, and exits cleanly.

All steps are logged to the audit trail.

### 6.6 Rollback Manager

If the newly started container does not reach a healthy running state, the rollback manager:

1. Stops the new container
2. Restores the previous image tag (the old image is always retained until the new container is confirmed healthy, even when `--cleanup` is enabled)
3. Starts the original container from the previous image
4. Emits a rollback event to the audit log and notifier

**Failure conditions** are user-configurable. Any combination of the following can be enabled:

| Condition | Description | Default |
|-----------|-------------|---------|
| `non-zero-exit` | Container exits with a non-zero exit code immediately after start | Enabled |
| `healthcheck-failure` | Docker healthcheck reports unhealthy within a configurable timeout | Enabled |
| `startup-timeout` | Container does not reach `running` state within N seconds | Enabled |

The timeout for `healthcheck-failure` and `startup-timeout` is configurable (default: 30 seconds).

### 6.7 Scheduler

Polling and the inbound webhook server are independent and can be active simultaneously. Either or both may be configured at the same time:

#### Polling

- Runs the update check on a configurable interval (cron expression or simple duration, e.g., `5m`, `1h`)
- Default interval: 24 hours

#### Inbound Webhook

- Exposes a lightweight HTTP server on a configurable port (default: `8080`)
- A `POST /v1/update` request triggers an immediate update check
- Optionally scoped to a specific container or image via query parameter
- The endpoint can be secured with a shared secret token (Bearer auth)
- Concurrent update behaviour: when an inbound webhook request arrives while an update is already in progress, the following rules apply (in all cases a warning is logged describing the scenario):
  - **Targeted request, already being updated**: the request is ignored and success is returned immediately
  - **Targeted request, not being updated**: the request proceeds normally
  - **Full-scan request, full scan already in progress**: the request is ignored and success is returned immediately
  - **Full-scan request, one or more targeted updates in progress**: the full scan proceeds but skips any containers or images already being updated

#### Run-Once

- Performs a single update cycle and exits (see §6.5)
- Mutually exclusive with polling and webhook modes

### 6.8 Notification System

#### Batching

Notifications are batched per update cycle. All events from a single scan are accumulated and delivered together as one notification when the cycle completes, rather than one notification per container. A configurable delay (default: 0) can be applied between cycle completion and dispatch, useful for rate-limiting or debouncing.

#### Events

Notifications are sent on the following events:

- One or more containers successfully updated
- One or more containers detected as stale in monitor-only mode (global or per-container)
- Rollback triggered
- Update check errors

Notifications are suppressed if the cycle produced no updates, no stale detections, and no failures.

#### Supported Targets

| Target | Transport | Notes |
|--------|-----------|-------|
| Webhook | HTTP POST | Generic JSON payload; configurable URL and headers |
| Email | SMTP | STARTTLS by default; configurable server, port, credentials |
| MQTT | TCP/TLS | Configurable broker URL, topic, QoS, and credentials |

Multiple targets can be active simultaneously.

#### Notification Payload

The webhook notification payload is a JSON object with the following structure:

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

`outcome` is one of:

| Value | Description |
|-------|-------------|
| `updated` | Image was pulled and container restarted successfully |
| `stale_detected` | Update detected in monitor-only mode; no restart performed |
| `rolled_back` | Container started but failed health checks; previous image restored |
| `failed` | Update attempted but did not succeed and rollback was not possible |

`old_image` and `new_image` are fully-qualified image references in `name:tag@sha256:digest` form. For a `stale_detected` outcome, `new_image` reflects the digest currently resolved by the registry; no pull has occurred. For a `failed` outcome where the failure occurred before the pull completed, `new_image` may be `null`.

#### Custom Templates

Notification templates use [MiniJinja](https://github.com/mitsuhiko/minijinja) (Jinja2-compatible) syntax. The template context exposes the same fields as the webhook JSON payload: `timestamp`, `hostname`, `summary` (with `scanned`, `updated`, `stale_detected`, `failed`), and `containers` (a list of objects with `name`, `old_image`, `new_image`, and `outcome`).

The built-in default template is:

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

Users may supply a custom template string via `--notification-template` / `SAURRON_NOTIFICATION_TEMPLATE` / `notifications.general.template` to replace the default.

### 6.9 Structured Logging & Audit Trail

- All log output is structured; supported formats: `json`, `logfmt`, `pretty` (human-readable), `auto` (pretty when connected to a TTY, logfmt otherwise)
- Log levels: `trace`, `debug`, `info`, `warn`, `error`; shorthand `--debug` and `--trace` flags available
- An **audit trail** records every update and rollback event with full context:
  - Timestamp
  - Container name and ID
  - Previous image digest and tag
  - New image digest and tag
  - Outcome (success / rolled back / failed)
  - Failure reason (if applicable)
- Audit entries are written to a dedicated append-only log file (path configurable) in addition to the main log stream

### 6.10 Build Version Metadata

- A version string is injected at build time (defaulting to `v0.0.0-unknown` for local builds)
- The version is included in startup log output
- The version drives the `User-Agent` header sent to registries (see §6.3)

---

## 7. Container Selection

### Default Behaviour

All containers are candidates for update by default, including running containers. Stopped and restarting containers can be included via flags.

### Inclusion Flags

| Flag | Description |
|------|-------------|
| `--include-stopped` | Include containers in `created` or `exited` state |
| `--include-restarting` | Include containers in `restarting` state |

### Opt-Out (Default Mode)

Containers can be excluded by setting the opt-out label to `false`:

```
saurron.enable=false
```

Containers can also be excluded by name using the `--disable-containers` flag (comma-separated list of container names).

### Opt-In Mode (`--label-enable`)

When this flag is set, only containers that explicitly set `saurron.enable=true` are included. All others are ignored.

### Per-Container Configuration Labels

| Label | Values | Description |
|-------|--------|-------------|
| `saurron.enable` | `true` / `false` | Include or exclude this container (or mark as opt-in when `--label-enable` is active) |
| `saurron.monitor-only` | `true` / `false` | Detect updates and notify but do not restart this container |
| `saurron.no-pull` | `true` / `false` | Restart from the cached image without pulling a new one |
| `saurron.stop-signal` | signal name (e.g., `SIGHUP`) | Override the stop signal sent to this container |
| `saurron.depends-on` | comma-separated container names | Declare explicit dependencies in addition to those inferred from Docker `--link` and `network_mode: container:` |
| `saurron.non-semver-strategy` | `skip` / `digest` | Override the non-semver tag strategy for this container. Default is `digest` (compare manifests); set to `skip` to disable update checks for this container's non-semver tag |
| `saurron.semver-pre-release` | `true` / `false` | When `true`, include pre-release versions (e.g., `1.2.3-beta`) when selecting the latest semver tag. Has no effect on non-semver tags. Default: `false` |
| `saurron.stop-timeout` | duration (e.g., `30s`) | Override graceful stop timeout for this container |

### Label Precedence

By default, per-container label values for `monitor-only` and `no-pull` take precedence over the corresponding global flags. Set `--global-takes-precedence` to invert this so that global flags override per-container labels.

---

## 8. HTTP API

The HTTP API server starts on a configurable port (default: `8080`) when any API feature is enabled. All endpoints except `GET /v1/health` require Bearer token authentication (`Authorization: Bearer <token>`). If a token is not configured, the process exits with an error at startup.

The `GET /v1/metrics` endpoint can be made unauthenticated via `--http-api-metrics-no-auth`, which is useful when Prometheus scrapes from a trusted network without bearer token support.

### `POST /v1/update`

Trigger an immediate update check.

**Query parameters (optional):**
- `?image=myorg/myapp` — restrict the update to containers using this image (comma-separated for multiple)
- `?container=mycontainer` — restrict the update to a specific container by name (comma-separated for multiple)

**Response:**
```json
{
  "status": "triggered",
  "timestamp": "2026-04-13T10:00:00Z"
}
```

The `status` field reflects the outcome of the request:

- `"triggered"` — an update cycle was started
- `"skipped"` — the request was dropped because the target (or a full scan) was already in progress
- `"merged"` — a full-scan request was accepted but one or more targeted containers were already being updated and were excluded from this cycle

See §6.7 for concurrency behaviour when a request arrives while an update is already in progress. The response is always `200 OK`.

### `GET /v1/metrics`

Serves Prometheus metrics in the standard text exposition format.

Tracked metrics:

| Metric | Type | Description |
|--------|------|-------------|
| `saurron_scans_total` | Counter | Total number of update cycles run |
| `saurron_scans_skipped` | Counter | Cycles skipped due to a concurrent update already running |
| `saurron_containers_scanned` | Gauge | Containers checked in the last cycle |
| `saurron_containers_updated` | Gauge | Containers updated in the last cycle |
| `saurron_containers_failed` | Gauge | Containers that failed to update in the last cycle |

### `GET /v1/health`

Returns `200 OK` when the service is running. Suitable for use as a Docker healthcheck. This endpoint is unauthenticated and does not require a Bearer token.

---

## 9. Technology Stack

| Concern | Choice | Rationale |
|---------|--------|-----------|
| Language | Rust (stable) | Memory safety, performance, strong async ecosystem |
| Async runtime | Tokio | De-facto standard; excellent ecosystem support |
| Docker API | `bollard` | Leading async Rust Docker client; actively maintained, hyper 1.x, full API coverage |
| HTTP server (webhook + API) | `axum` | Tokio-native, ergonomic, well-maintained |
| HTTP client (registry) | `reqwest` | Tokio-native, widely used |
| Email | `lettre` | Pure Rust SMTP client with STARTTLS support |
| MQTT | `rumqttc` | Async Rust MQTT client |
| Prometheus metrics | `prometheus` | Standard Rust Prometheus client |
| Config | `config-rs` + `clap` | Layered config (file + env + CLI) |
| Logging | `tracing` + `tracing-subscriber` | Structured, async-aware logging |
| Serialization | `serde` + `serde_json` | Standard Rust serialization |
| Notification templates | `minijinja` | Runtime Jinja2-style templating; single required dependency (`serde`) |
| SemVer parsing | `semver` | Official SemVer crate from the Cargo ecosystem |

---

## 10. Configuration Reference

> All environment variables use the prefix `SAURRON_`.

### 10.1 General

| Purpose | CLI Flag | Environment Variable | TOML Key |
|---------|----------|----------------------|----------|
| Path to the TOML config file | `--config <path>` | `SAURRON_CONFIG` | *(not applicable)* |
| Log level (`trace`, `debug`, `info`, `warn`, `error`). Default: `info` | `--log-level <level>` | `SAURRON_LOG_LEVEL` | `log_level` |
| Log format (`auto`, `json`, `logfmt`, `pretty`). Default: `auto` | `--log-format <format>` | `SAURRON_LOG_FORMAT` | `log_format` |
| Shorthand for `--log-level debug` | `--debug` | — | — |
| Shorthand for `--log-level trace` | `--trace` | — | — |
| Path to the append-only audit log file | `--audit-log <path>` | `SAURRON_AUDIT_LOG` | `audit_log` |

### 10.2 Docker Connection

| Purpose | CLI Flag | Environment Variable | TOML Key |
|---------|----------|----------------------|----------|
| Docker daemon socket or host URL. Default: `unix:///var/run/docker.sock` | `--host <uri>` | `DOCKER_HOST` | `docker.host` |
| Enable TLS for the Docker daemon connection | `--tlsverify` | `DOCKER_TLS_VERIFY` | `docker.tls_verify` |
| Path to the TLS CA certificate | `--tls-ca-cert <path>` | `DOCKER_CERT_PATH` | `docker.tls_ca_cert` |
| Path to the TLS client certificate | `--tls-cert <path>` | — | `docker.tls_cert` |
| Path to the TLS client key | `--tls-key <path>` | — | `docker.tls_key` |
| Docker API version to negotiate. Default: auto-negotiate | `--api-version <version>` | `DOCKER_API_VERSION` | `docker.api_version` |

### 10.3 Scheduling

| Purpose | CLI Flag | Environment Variable | TOML Key |
|---------|----------|----------------------|----------|
| Poll interval as a duration (e.g., `5m`, `1h`). Converted to a cron expression internally. Mutually exclusive with `--schedule`. Default: `24h` | `--interval <duration>` | `SAURRON_POLL_INTERVAL` | `poll_interval` |
| Poll schedule as a cron expression (e.g., `0 4 * * *`). Mutually exclusive with `--interval` | `--schedule <cron>` | `SAURRON_SCHEDULE` | `schedule` |
| Perform a single update cycle and exit. Mutually exclusive with `--interval` and `--schedule` | `--run-once` | `SAURRON_RUN_ONCE` | `run_once` |

### 10.4 Container Selection

| Purpose | CLI Flag | Environment Variable | TOML Key |
|---------|----------|----------------------|----------|
| Switch to opt-in mode: only update containers that have `saurron.enable=true` | `--label-enable` | `SAURRON_LABEL_ENABLE` | `label_enable` |
| Comma-separated list of container names to always exclude from updates | `--disable-containers <names>` | `SAURRON_DISABLE_CONTAINERS` | `disable_containers` |
| Include containers in `created` or `exited` state | `--include-stopped` | `SAURRON_INCLUDE_STOPPED` | `include_stopped` |
| Include containers in `restarting` state | `--include-restarting` | `SAURRON_INCLUDE_RESTARTING` | `include_restarting` |
| Make global flags take precedence over per-container labels for `monitor-only` and `no-pull` (default is label precedence) | `--global-takes-precedence` | `SAURRON_GLOBAL_TAKES_PRECEDENCE` | `global_takes_precedence` |

### 10.5 Update Strategy

| Purpose | CLI Flag | Environment Variable | TOML Key |
|---------|----------|----------------------|----------|
| Detect updates and notify but do not pull or restart any container | `--monitor-only` | `SAURRON_MONITOR_ONLY` | `monitor_only` |
| Skip pulling a new image; restart containers using the cached image | `--no-pull` | `SAURRON_NO_PULL` | `no_pull` |
| Remove old images after a successful update | `--cleanup` | `SAURRON_CLEANUP` | `cleanup` |
| Start stopped containers after updating their image | `--revive-stopped` | `SAURRON_REVIVE_STOPPED` | `revive_stopped` |
| Time to wait for a container to stop gracefully before sending SIGKILL. Default: `10s` | `--stop-timeout <duration>` | `SAURRON_STOP_TIMEOUT` | `stop_timeout` |

### 10.6 Rollback

| Purpose | CLI Flag | Environment Variable | TOML Key |
|---------|----------|----------------------|----------|
| Trigger a rollback if the new container exits with a non-zero code. Default: enabled | `--rollback-on-exit-code` / `--no-rollback-on-exit-code` | `SAURRON_ROLLBACK_ON_EXIT_CODE` | `rollback.on_exit_code` |
| Trigger a rollback if the Docker healthcheck reports unhealthy within the startup timeout. Default: enabled | `--rollback-on-healthcheck` / `--no-rollback-on-healthcheck` | `SAURRON_ROLLBACK_ON_HEALTHCHECK` | `rollback.on_healthcheck` |
| Trigger a rollback if the container does not reach `running` state within the startup timeout. Default: enabled | `--rollback-on-timeout` / `--no-rollback-on-timeout` | `SAURRON_ROLLBACK_ON_TIMEOUT` | `rollback.on_timeout` |
| How long to wait for a container to become healthy or reach `running` before triggering a rollback. Default: `30s` | `--startup-timeout <duration>` | `SAURRON_STARTUP_TIMEOUT` | `rollback.startup_timeout` |

### 10.7 Registry

| Purpose | CLI Flag | Environment Variable | TOML Key |
|---------|----------|----------------------|----------|
| Warning behaviour for failed registry HEAD requests: `auto` (default — warn only for Docker Hub and ghcr.io), `always`, `never` | `--head-warn-strategy <strategy>` | `SAURRON_HEAD_WARN_STRATEGY` | `head_warn_strategy` |

### 10.8 HTTP API

| Purpose | CLI Flag | Environment Variable | TOML Key |
|---------|----------|----------------------|----------|
| Enable the on-demand update trigger endpoint (`POST /v1/update`) | `--http-api-update` | `SAURRON_HTTP_API_UPDATE` | `http_api.update` |
| Enable the Prometheus metrics endpoint (`GET /v1/metrics`) | `--http-api-metrics` | `SAURRON_HTTP_API_METRICS` | `http_api.metrics` |
| Bearer token required for all API requests | `--http-api-token <token>` | `SAURRON_HTTP_API_TOKEN` | `http_api.token` |
| Port the HTTP API server listens on. Default: `8080` | `--http-api-port <port>` | `SAURRON_HTTP_API_PORT` | `http_api.port` |
| Serve `GET /v1/metrics` without requiring a Bearer token. Default: `false` | `--http-api-metrics-no-auth` | `SAURRON_HTTP_API_METRICS_NO_AUTH` | `http_api.metrics_no_auth` |

### 10.9 Notifications — General

| Purpose | CLI Flag | Environment Variable | TOML Key |
|---------|----------|----------------------|----------|
| Delay between update cycle completion and notification dispatch (e.g., `30s`). Default: `0s` | `--notification-delay <duration>` | `SAURRON_NOTIFICATION_DELAY` | `notifications.general.delay` |
| Custom notification template string; uses the built-in default template when omitted | `--notification-template <template>` | `SAURRON_NOTIFICATION_TEMPLATE` | `notifications.general.template` |

### 10.10 Notifications — Webhook

| Purpose | CLI Flag | Environment Variable | TOML Key |
|---------|----------|----------------------|----------|
| URL to POST notification payloads to | `--webhook-url <url>` | `SAURRON_WEBHOOK_URL` | `notifications.webhook.url` |
| Additional HTTP headers included in every webhook request, as comma-separated `Key:Value` pairs | `--webhook-headers <headers>` | `SAURRON_WEBHOOK_HEADERS` | `notifications.webhook.headers` |
| Skip TLS certificate verification for the webhook endpoint. Default: `false` — an invalid certificate logs an error and skips the webhook | `--webhook-tls-skip-verify` | `SAURRON_WEBHOOK_TLS_SKIP_VERIFY` | `notifications.webhook.tls_skip_verify` |

### 10.11 Notifications — Email

| Purpose | CLI Flag | Environment Variable | TOML Key |
|---------|----------|----------------------|----------|
| Sender address | `--notification-email-from <address>` | `SAURRON_NOTIFICATION_EMAIL_FROM` | `notifications.email.from` |
| Recipient address(es), comma-separated | `--notification-email-to <addresses>` | `SAURRON_NOTIFICATION_EMAIL_TO` | `notifications.email.to` |
| SMTP server hostname | `--notification-email-server <host>` | `SAURRON_NOTIFICATION_EMAIL_SERVER` | `notifications.email.server` |
| SMTP server port. Default: `587` | `--notification-email-port <port>` | `SAURRON_NOTIFICATION_EMAIL_PORT` | `notifications.email.port` |
| SMTP authentication username | `--notification-email-user <user>` | `SAURRON_NOTIFICATION_EMAIL_USER` | `notifications.email.user` |
| SMTP authentication password | `--notification-email-password <password>` | `SAURRON_NOTIFICATION_EMAIL_PASSWORD` | `notifications.email.password` |
| Skip TLS certificate verification for the SMTP connection | `--notification-email-tls-skip-verify` | `SAURRON_NOTIFICATION_EMAIL_TLS_SKIP_VERIFY` | `notifications.email.tls_skip_verify` |

### 10.12 Notifications — MQTT

| Purpose | CLI Flag | Environment Variable | TOML Key |
|---------|----------|----------------------|----------|
| MQTT broker URL (e.g., `tcp://broker.example.com:1883` or `ssl://broker.example.com:8883`) | `--notification-mqtt-broker <url>` | `SAURRON_NOTIFICATION_MQTT_BROKER` | `notifications.mqtt.broker` |
| MQTT topic to publish notifications to | `--notification-mqtt-topic <topic>` | `SAURRON_NOTIFICATION_MQTT_TOPIC` | `notifications.mqtt.topic` |
| MQTT QoS level: `0` (at most once), `1` (at least once), or `2` (exactly once). Default: `0` | `--notification-mqtt-qos <level>` | `SAURRON_NOTIFICATION_MQTT_QOS` | `notifications.mqtt.qos` |
| MQTT client identifier; auto-generated if omitted | `--notification-mqtt-client-id <id>` | `SAURRON_NOTIFICATION_MQTT_CLIENT_ID` | `notifications.mqtt.client_id` |
| MQTT broker authentication username | `--notification-mqtt-username <user>` | `SAURRON_NOTIFICATION_MQTT_USERNAME` | `notifications.mqtt.username` |
| MQTT broker authentication password | `--notification-mqtt-password <password>` | `SAURRON_NOTIFICATION_MQTT_PASSWORD` | `notifications.mqtt.password` |

---

## 11. Future Enhancements

The following features are explicitly deferred from the initial release and tracked here for future planning:

| Feature | Notes |
|---------|-------|
| Docker Swarm support | Multi-host orchestration |
| Kubernetes support | Controller/operator model |
| Private registry authentication | Docker config file + env var credential sources |
| Dependent container restarts | Restart containers sharing networks or volumes with an updated container |
| Slack notifications | |
| Microsoft Teams notifications | |
| Gotify notifications | |
| Discord notifications | |
| Docker Hub inbound webhook format | Parse Docker Hub-specific webhook payloads |
| Web UI | Dashboard for update history and manual triggers |
| Lifecycle hooks | Pre/post-check and pre/post-update shell commands run inside containers; `EX_TEMPFAIL` exit code to signal skip-without-failure |
| Notification template preview | A mode to validate custom notification templates against synthetic data without running a real update cycle |
| Scope-based multi-instance support | Multiple tool instances on the same host managing non-overlapping sets of containers via a scope label |
| Multiple instance detection | Detect duplicate instances sharing the same scope and stop all but the most recently created |

---

*Document status: Draft*
