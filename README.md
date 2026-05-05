<div align="center">

<img src="./logo.png" width="450" />

# Saurron

[![Build Status](https://github.com/organicveggie/saurron/actions/workflows/rust.yml/badge.svg)](https://github.com/organicveggie/saurron/actions/workflows/rust.yml) [![Coverage Status](https://coveralls.io/repos/github/organicveggie/saurron/badge.svg?branch=main)](https://coveralls.io/github/organicveggie/saurron?branch=main) [![License](https://img.shields.io/github/license/organicveggie/saurron)](https://github.com/organicveggie/saurron/blob/master/LICENSE) [![Docker](https://ghcr-badge.egpl.dev/organicveggie/saurron/tags?label=ghcr.io&trim=major)](https://github.com/organicveggie/saurron/pkgs/container/saurron) [![Static Badge](https://img.shields.io/badge/docs-sauron.io-orange?link=https%3A%2F%2Fdocs.saurron.io)](https://docs.saurron.io)


**Ever-watchful eye for your Docker containers.** Saurron monitors containers on a single host, detects newer images, and automatically updates them — with safe rollback, structured audit logging, and flexible notifications.

Inspired by [Watchtower](https://github.com/containrrr/watchtower).
</div>

---

# Features

- **Audit trail** - Append-only JSON log file capturing every update and rollback event
    (container name/id, old and new image digests and tags, outcome, failure reason).
- **Container enumeration** - Lists running, restarting, and stopped containers with full
    opt-in / opt-out selection logic.
- **Container selection** - Per-container overrides through labels. Allow-list. Exclusion list.
    Opt-in mode.
- **Docker client** - Connects via Unix socket or TLS-secured TCP. Configurable API version.
    Verifies daemon reachability on startup.
- **Freshness detection** - SemVer tag ranking with strict 2.0.0 grammar. Non-SemVer digest comparison.
    Per-container overrides. Configurable `--head-warn-strategy` for failed manifest HEAD requests.
- **Graceful shutdown** - `SIGTERM`/`SIGINT` wait for in-progress update cycle to complete before
    exiting.
- **HTTP API** - Update specific images or containers. Health check. Metrics. Bearer token auth.
    Concurrent-request locking. Structured JSON access log per request.
- **Layered configuration** - TOML file, environment variables, and CLI flags.
- **Notifications** - Batched per-cycle reports dispatched to all configured targets concurrently.
    Trgets include: webhook (HTTP POST, custom headers, optional TLS skip-verify), email
    (SMTP/STARTTLS via lettre), MQTT (MQTTv5 via rumqttc, QoS 0/1/2), Pushover.
    MiniJinja template rendering with configurable custom template.
    User configurable notification criteria (any update, failure, or rollback).
- **Prometheus metrics** - `saurron_scan_cycles_total`, `saurron_scan_cycles_skipped_total` (HTTP
    409 conflicts), `saurron_containers_scanned_total`, `saurron_containers_updated_total`, and
    `saurron_containers_failed_total`.
- **Registry client** - Docker Registry HTTP API v2. Manifest HEAD for digest comparison. Bearer
    token auth for public registries. SemVer tag enumeration and highest-version selection.
    Pre-release opt-in via label. Digest-pinned image detection and skip.
- **Rollback manager** - Automatic rollback on non-zero exit, healthcheck failure, or startup
    timeout. Configurable timeout. Full audit log entries for rollback events.
- **Scheduler** - Poll interval (duration or cron expression). `--run-once` mode for external
    schedulers.
- **Self-update** - Detects own container via `$HOSTNAME`. rRenames running container, starts
    replacement under original name. Failure recovery restores previous container.
- **Structured logging** - Four output formats: `pretty` (colored TTY), `json` (newline-delimited),
    `logfmt` (key=value), `auto` (detects TTY); configurable log level; `RUST_LOG` override supported.
- **Update engine** - Pulls new image and preserves run config ((env, volumes, networks, ports,
    labels) before stopping old container. Uses preserved config when creating new container.
    Recreates one container at a time in reverse dependency order.

---

# Getting Started

## Pre-built binaries

Download a tarball for your platform from the [latest release](https://github.com/organicveggie/saurron/releases/latest):

```bash
# Linux amd64
curl -Lo saurron.tar.gz https://github.com/organicveggie/saurron/releases/latest/download/saurron-latest-linux-amd64.tar.gz
tar -xzf saurron.tar.gz
./saurron --help
```

## Build from source

```bash
cargo build --release
./target/release/saurron --help
```

## Docker

```bash
# Pull from GHCR
docker pull ghcr.io/organicveggie/saurron:latest

# Run — mount the Docker socket and a config file
docker run -d \
  --name saurron \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v /etc/saurron/config.toml:/etc/saurron/config.toml:ro \
  --group-add "$(stat -c '%g' /var/run/docker.sock)" \
  -e TZ=America/New_York \
  ghcr.io/organicveggie/saurron:latest

# Single update cycle
docker run --rm \
  -v /var/run/docker.sock:/var/run/docker.sock \
  --group-add "$(stat -c '%g' /var/run/docker.sock)" \
  ghcr.io/organicveggie/saurron:latest --run-once
```

The container runs as UID 1000 (non-root). The `--group-add` flag grants access
to the Docker socket by joining the socket's group on the host.

Published images are multi-platform (`linux/amd64`, `linux/arm64`). Available
tags: `latest`, `edge` (latest `main`), and semver pins (`v1`, `v1.2`, `v1.2.3`).

## Run from binary

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

## Configuration

Saurron accepts a TOML config file (default: `/etc/saurron/config.toml`), environment variables prefixed with `SAURRON_`, and CLI flags. CLI and env vars take precedence over the config file.

```toml
# /etc/saurron/config.toml
log_level = "info"
log_format = "json"
audit_log = "/var/log/saurron/audit.log"

[docker]
host = "unix:///var/run/docker.sock"
```

See https://docs.saurron.io/start/configuration.html for more information about the supported configuration options.

---

# Documentation

Read the full docs at https://docs.saurron.io.
