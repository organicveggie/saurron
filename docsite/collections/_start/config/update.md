---
layout: page
title: Update Strategy
parent: Configuration
---

<!-- prettier-ignore-start -->
# Update Strategy
{: .no_toc }

<!-- prettier-ignore-end -->

Settings to control how Saurron handles image updates. By default, stale containers are updated one
at a time in reverse dependency order — each container is fully stopped, recreated from the new
image, and confirmed healthy before the next one begins.

<!-- prettier-ignore-start -->
* TOC
{:toc}

<!-- prettier-ignore-end -->

## Cleanup

CLI flag
: `--cleanup`

Environment
: `SAURRON_CLEANUP`

TOML key
: `cleanup`

Remove the previous image after a container is successfully updated. The old image is always
retained until the new container is confirmed healthy so that rollback remains possible; cleanup
runs after that confirmation.

## Monitor only

CLI flag
: `--monitor-only`

Environment
: `SAURRON_MONITOR_ONLY`

TOML key
: `monitor_only`

Detect stale images and send notifications, but do not pull new images or restart containers. Useful
for observing what would be updated before enabling automatic updates.

This can also be set per-container via the `saurron.monitor-only` label. By default the label takes
precedence over this global flag; set `--global-takes-precedence` to invert that behaviour.

## No pull

CLI flag
: `--no-pull`

Environment
: `SAURRON_NO_PULL`

TOML key
: `no_pull`

Restart containers from the locally cached image without pulling a new one from the registry. Useful
when images are loaded out-of-band and pulling is not desired.

This can also be set per-container via the `saurron.no-pull` label. By default the label takes
precedence over this global flag; set `--global-takes-precedence` to invert that behaviour.

## Revive stopped

CLI flag
: `--revive-stopped`

Environment
: `SAURRON_REVIVE_STOPPED`

TOML key
: `revive_stopped`

Include containers in `created` or `exited` state as update candidates. When enabled, stopped
containers are treated the same as running ones: a new image is pulled, the container is recreated,
and it is started.

By default stopped containers are left untouched to avoid auto-restarting containers that were
intentionally stopped.

## Stop timeout

CLI flag
: `--stop-timeout <duration>`

Environment
: `SAURRON_STOP_TIMEOUT`

TOML key
: `stop_timeout`

How long to wait for a container to exit gracefully after sending the stop signal before sending
`SIGKILL`. Default: `10s`.

This can also be set per-container via the `saurron.stop-timeout` label.
