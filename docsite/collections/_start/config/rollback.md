---
layout: page
title: Rollbacks
parent: Configuration
---

<!-- prettier-ignore-start -->
# Rollback
{: .no_toc }

<!-- prettier-ignore-end -->

When a container fails to start successfully after an update, Saurron automatically rolls back to
the previous image. The rollback sequence is: stop the new container, restore the previous image
tag, start the original container, and emit a rollback event to the audit log and notifier.

The old image is always retained until the new container is confirmed healthy — even when
`--cleanup` is enabled — so that rollback is always possible.

All three rollback conditions are enabled by default. Any combination can be active simultaneously.

<!-- prettier-ignore-start -->
* TOC
{:toc}

<!-- prettier-ignore-end -->

## Rollback on exit code

CLI flag
: `--rollback-on-exit-code` / `--no-rollback-on-exit-code`

Environment
: `SAURRON_ROLLBACK_ON_EXIT_CODE`

TOML key
: `rollback.on_exit_code`

Trigger a rollback if the new container exits with a non-zero exit code immediately after starting.
Default: enabled.

## Rollback on healthcheck failure

CLI flag
: `--rollback-on-healthcheck` / `--no-rollback-on-healthcheck`

Environment
: `SAURRON_ROLLBACK_ON_HEALTHCHECK`

TOML key
: `rollback.on_healthcheck`

Trigger a rollback if the Docker healthcheck reports the new container as unhealthy within the
startup timeout window. Only applies to containers that have a Docker healthcheck configured.
Default: enabled.

## Rollback on startup timeout

CLI flag
: `--rollback-on-timeout` / `--no-rollback-on-timeout`

Environment
: `SAURRON_ROLLBACK_ON_TIMEOUT`

TOML key
: `rollback.on_timeout`

Trigger a rollback if the new container does not reach `running` state within the startup timeout
window. Default: enabled.

## Startup timeout

CLI flag
: `--startup-timeout <duration>`

Environment
: `SAURRON_STARTUP_TIMEOUT`

TOML key
: `rollback.startup_timeout`

How long to wait for the new container to reach a healthy running state before triggering a
rollback. Applies to both the healthcheck failure and startup timeout conditions. Default: `30s`.
