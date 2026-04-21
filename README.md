<div align="center">

<img src="./logo.png" width="450" />

# Saurron

[![Build Status](https://github.com/organicveggie/saurron/actions/workflows/rust.yml/badge.svg)](https://github.com/organicveggie/saurron/actions/workflows/rust.yml) [![Coverage Status](https://coveralls.io/repos/github/organicveggie/saurron/badge.svg?branch=coverage)](https://coveralls.io/github/organicveggie/saurron?branch=coverage) [![License](https://img.shields.io/github/license/organicveggie/saurron)](https://github.com/organicveggie/saurron/blob/master/LICENSE)

**Ever-watchful eye for your Docker containers.** Saurron monitors containers on a single host, detects newer images, and automatically updates them — with safe rollback, structured audit logging, and flexible notifications.

Inspired by [Watchtower](https://github.com/containrrr/watchtower).
</div>

---

## Features

### Implemented

- **Configuration layer** — layered config from TOML file, environment variables, and CLI flags with correct precedence; Docker secrets support via file path resolution
- **Docker client** — connects via Unix socket or TLS-secured TCP; configurable API version; verifies daemon reachability on startup
- **Container enumeration** — lists running, restarting, and stopped containers with full opt-in / opt-out selection logic
- **Container selection** — per-container label overrides (`saurron.enable`), `--containers` allow-list, `--disable-containers` exclusion list, `--label-enable` opt-in mode, `--global-takes-precedence` flag
- **Structured logging** — four output formats: `pretty` (colored TTY), `json` (newline-delimited), `logfmt` (key=value), `auto` (detects TTY); configurable log level; `RUST_LOG` override supported
- **Audit trail** — append-only JSON log file capturing every update and rollback event (container name/id, old and new image digests and tags, outcome, failure reason)

### Planned

- **Registry client** — Docker Registry HTTP API v2; manifest HEAD requests for digest comparison; SemVer tag enumeration and highest-version selection; pre-release opt-in; digest-pinned image detection
- **Update engine** — pull new image → stop old container → start with preserved config (env, volumes, networks, ports, labels) → one container at a time in reverse dependency order; `--monitor-only`, `--no-pull`, `--cleanup`, `--revive-stopped` modes
- **Rollback manager** — automatic rollback on non-zero exit, healthcheck failure, or startup timeout; configurable timeout; full audit log entries for rollback events
- **Scheduler** — poll interval (duration or cron expression); `--run-once` mode for external schedulers; inbound webhook with Bearer token auth and concurrent-request rules
- **HTTP API** — `POST /v1/update` (with `?image=` and `?container=` scoping), `GET /v1/health`, `GET /v1/metrics`
- **Self-update** — detects own container via `$HOSTNAME`; renames running container, starts replacement under original name; failure recovery restores previous container
- **Graceful shutdown** — `SIGTERM`/`SIGINT` finishes in-progress update cycle before exiting
- **Notifications** — batched per-cycle reports via webhook (HTTP POST), email (SMTP/STARTTLS), MQTT, and Pushover; MiniJinja custom templates
- **Prometheus metrics** — five metrics exposed at `/v1/metrics`: scans total/skipped, containers scanned/updated/failed
- **Containerization** — multi-stage `Dockerfile`; integration tests against a `docker-compose` fixture with a local registry

---

## Getting Started

### Prerequisites

- Rust (stable toolchain)
- Docker daemon accessible via Unix socket or TCP

### Build

```bash
cargo build --release
```

### Run

```bash
# Connect to local Docker daemon and enumerate containers
./target/release/saurron

# Single update cycle, then exit
./target/release/saurron --run-once

# Monitor only — detect stale images but do not update
./target/release/saurron --monitor-only

# JSON log output
./target/release/saurron --log-format json

# Write audit events to a dedicated file
./target/release/saurron --audit-log /var/log/saurron/audit.log
```

### Configuration

Saurron accepts a TOML config file (default: `/etc/saurron/config.toml`), environment variables prefixed with `SAURRON_`, and CLI flags. CLI and env vars take precedence over the config file.

```toml
# /etc/saurron/config.toml
log_level = "info"
log_format = "json"
audit_log = "/var/log/saurron/audit.log"

[docker]
host = "unix:///var/run/docker.sock"
```

Run `saurron --help` for a full list of flags.

---

## Container Labels

Control per-container behaviour with Docker labels:

| Label                        | Values           | Description                      |
| ---------------------------- | ---------------- | -------------------------------- |
| `saurron.enable`             | `true` / `false` | Include or exclude from updates  |
| `saurron.monitor-only`       | `true` / `false` | Detect updates but never restart |
| `saurron.no-pull`            | `true` / `false` | Restart from cached image        |
| `saurron.stop-signal`        | e.g. `SIGHUP`    | Override stop signal             |
| `saurron.stop-timeout`       | e.g. `30s`       | Override graceful stop timeout   |
| `saurron.depends-on`         | container names  | Explicit dependency ordering     |
| `saurron.semver-pre-release` | `true` / `false` | Include pre-release versions     |
