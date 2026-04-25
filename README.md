<div align="center">

<img src="./logo.png" width="450" />

# Saurron

[![Build Status](https://github.com/organicveggie/saurron/actions/workflows/rust.yml/badge.svg)](https://github.com/organicveggie/saurron/actions/workflows/rust.yml) [![Coverage Status](https://coveralls.io/repos/github/organicveggie/saurron/badge.svg?branch=main)](https://coveralls.io/github/organicveggie/saurron?branch=main) [![License](https://img.shields.io/github/license/organicveggie/saurron)](https://github.com/organicveggie/saurron/blob/master/LICENSE) [![Docker](https://ghcr-badge.egpl.dev/organicveggie/saurron/tags?label=ghcr.io&trim=major)](https://github.com/organicveggie/saurron/pkgs/container/saurron)

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
- **Registry client** — Docker Registry HTTP API v2; manifest HEAD for digest comparison; Bearer token auth for public registries; SemVer tag enumeration and highest-version selection; pre-release opt-in via `saurron.semver-pre-release` label; digest-pinned image detection and skip
- **Freshness detection** — non-SemVer digest comparison, SemVer tag ranking with strict 2.0.0 grammar, per-container `saurron.semver-pre-release` and `saurron.non-semver-strategy` label overrides; configurable `--head-warn-strategy` for failed manifest HEAD requests
- **Update engine** — pull new image → stop old container → start with preserved run config (env, volumes, networks, ports, labels) → one container at a time in reverse dependency order; `--monitor-only`, `--no-pull`, `--cleanup`, `--revive-stopped` modes
- **Rollback manager** — automatic rollback on non-zero exit, healthcheck failure, or startup timeout; configurable timeout; full audit log entries for rollback events
- **Scheduler** — poll interval (duration or cron expression); `--run-once` mode for external schedulers
- **HTTP API** — `POST /v1/update` (with `?image=` and `?container=` scoping), `GET /v1/health`, `GET /v1/metrics`; Bearer token auth; concurrent-request locking
- **Self-update** — detects own container via `$HOSTNAME`; renames running container, starts replacement under original name; failure recovery restores previous container
- **Graceful shutdown** — `SIGTERM`/`SIGINT` waits for in-progress update cycle to complete before exiting
- **Notifications** — batched per-cycle reports dispatched to all configured targets concurrently; targets: webhook (HTTP POST, custom headers, optional TLS skip-verify), email (SMTP/STARTTLS via lettre), MQTT (MQTTv5 via rumqttc, QoS 0/1/2), Pushover; MiniJinja template rendering with configurable custom template; fires only on interesting cycles (any update, failure, or rollback)
- **Prometheus metrics** — five `IntCounter` metrics exposed at `GET /v1/metrics`: `saurron_scan_cycles_total`, `saurron_scan_cycles_skipped_total` (HTTP 409 conflicts), `saurron_containers_scanned_total`, `saurron_containers_updated_total`, `saurron_containers_failed_total`

---

## Getting Started

### Build from source

```bash
cargo build --release
./target/release/saurron --help
```

### Docker

```bash
# Pull from GHCR
docker pull ghcr.io/organicveggie/saurron:latest

# Run — mount the Docker socket and a config file
docker run -d \
  --name saurron \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v /etc/saurron/config.toml:/etc/saurron/config.toml:ro \
  --group-add "$(stat -c '%g' /var/run/docker.sock)" \
  ghcr.io/organicveggie/saurron:latest

# Single update cycle
docker run --rm \
  -v /var/run/docker.sock:/var/run/docker.sock \
  --group-add "$(stat -c '%g' /var/run/docker.sock)" \
  ghcr.io/organicveggie/saurron:latest --run-once

# Build from source (injects version string)
docker build --build-arg SAURRON_BUILD_VERSION=v1.0.0 -t saurron:v1.0.0 .
```

> The container runs as UID 1000 (non-root). The `--group-add` flag grants access
> to the Docker socket by joining the socket's group on the host.
>
> Published images are multi-platform (`linux/amd64`, `linux/arm64`). Available
> tags: `latest`, `edge` (latest `main`), and semver pins (`v1`, `v1.2`, `v1.2.3`).

### Run from binary

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
