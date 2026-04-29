---
layout: page
title: What is Saurron?
nav_order: 2
---

# What is Saurron?

Saurron is a service which monitors containers on a single host, detects newer images, and
automatically updates them - with safe rollback, structured audit logging, and flexible
notifications.

# Features

- **Audit trail** - Append-only JSON log file capturing every update and rollback event
    (container name/id, old and new image digests and tags, outcome, failure reason).
- **Container enumeration** - Lists running, restarting, and stopped containers with full
    opt-in / opt-out selection logic.
- **Container selection** - Per-container overrides through labels. Allow-list. Exclusion list.
    Opt-in mode.
- **Docker client** - Connects via Unix socket or TLS-secured TCP. Configurable API version.
    Verifies daemon reachability on startup.
- **Docker secrets support** - TODO
- **Freshness detection** - SemVer tag ranking with strict 2.0.0 grammar. Non-SemVer digest comparison.
    Per-container overrides. Configurable `--head-warn-strategy` for failed manifest HEAD requests.
- **Graceful shutdown** - `SIGTERM`/`SIGINT` wait for in-progress update cycle to complete before
    exiting.
- **HTTP API** - Update specific images or containres. Health check. Metrics. Bearer token auth.
    Concurrent-request locking.
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
