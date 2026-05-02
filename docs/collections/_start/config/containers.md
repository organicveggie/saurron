---
layout: page
title: Containers
parent: Configuration
---

<!-- prettier-ignore-start -->
# Container Selection
{: .no_toc }

<!-- prettier-ignore-end -->

By default, Saurron considers all running containers as candidates for update. Stopped and
restarting containers are excluded unless explicitly opted in. The options below control which
containers are selected, how per-container labels interact with global flags, and how individual
containers can override behaviour via Docker labels.

<!-- prettier-ignore-start -->
* TOC
{:toc}

<!-- prettier-ignore-end -->

## Containers allow-list

CLI flag
: `--containers <names>`

Environment
: `SAURRON_CONTAINERS`

TOML key
: `containers`

Comma-separated list of container names to consider. When set, all other containers are ignored.
State filters (`--revive-stopped`, `--include-restarting`), `--disable-containers`, and label
checks still apply on top of this allow-list.

## Disable containers

CLI flag
: `--disable-containers <names>`

Environment
: `SAURRON_DISABLE_CONTAINERS`

TOML key
: `disable_containers`

Comma-separated list of container names to always exclude from update checks, regardless of other
settings.

## Global takes precedence

CLI flag
: `--global-takes-precedence`

Environment
: `SAURRON_GLOBAL_TAKES_PRECEDENCE`

TOML key
: `global_takes_precedence`

By default, per-container labels take precedence over global flags for `monitor-only` and `no-pull`.
Enable this flag to invert that behaviour so that global flags win over container labels.

## Include restarting

CLI flag
: `--include-restarting`

Environment
: `SAURRON_INCLUDE_RESTARTING`

TOML key
: `include_restarting`

Include containers in `restarting` state as candidates for update. By default these containers are
excluded.

## Label enable (opt-in mode)

CLI flag
: `--label-enable`

Environment
: `SAURRON_LABEL_ENABLE`

TOML key
: `label_enable`

Switch from opt-out to opt-in mode. When enabled, only containers with the label
`saurron.enable=true` are considered for updates. All other containers are ignored.

Without this flag (the default), all running containers are included unless explicitly excluded via
`saurron.enable=false` or `--disable-containers`.

## Per-container labels

Individual containers can override global settings by setting labels with the `saurron.` prefix.
Labels are read from the container at the start of each update cycle.

### saurron.enable

Values
: `true` / `false`

Explicitly include or exclude a container. In default (opt-out) mode, set `false` to exclude. In
opt-in mode (`--label-enable`), set `true` to include.

### saurron.depends-on

Values
: comma-separated container names

Declare explicit dependencies beyond what Docker `--link` and `network_mode: container:` provide.
Dependent containers are updated before the containers they depend on (reverse dependency order).

### saurron.monitor-only

Values
: `true` / `false`

Detect and notify about updates for this container without pulling or restarting. Equivalent to the
global `--monitor-only` flag but scoped to a single container.

### saurron.no-pull

Values
: `true` / `false`

Restart this container from the locally cached image without pulling a new one. Equivalent to the
global `--no-pull` flag but scoped to a single container.

### saurron.non-semver-strategy

Values
: `digest` / `skip`

Override the freshness detection strategy for non-semver tags (e.g. `latest`, `stable`). The
default `digest` compares the running image's manifest digest against the digest currently resolved
by the registry. Set `skip` to disable update checks for this container entirely when using a
non-semver tag.

### saurron.semver-pre-release

Values
: `true` / `false`

Include pre-release versions (e.g. `1.2.3-beta`) when selecting the latest semver tag for this
container. Has no effect on non-semver tags. Default: `false`.

### saurron.stop-signal

Values
: signal name (e.g. `SIGHUP`, `SIGTERM`)

Override the stop signal sent to this container when stopping it for an update. Overrides the
signal set in the container's image or Docker configuration.

### saurron.stop-timeout

Values
: duration (e.g. `30s`, `1m`)

Override the graceful stop timeout for this container. Saurron waits this long for the container
to exit after sending the stop signal before sending `SIGKILL`.
