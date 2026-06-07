---
layout: page
title: Registry
parent: Configuration
---

<!-- prettier-ignore-start -->
# Registry
{: .no_toc }

<!-- prettier-ignore-end -->

Settings to control how Saurron communicates with image registries. Saurron uses the Docker
Registry HTTP API v2 to fetch manifests and compare digests without pulling images. Credentials can
be set globally or scoped per registry.

<!-- prettier-ignore-start -->
* TOC
{:toc}

<!-- prettier-ignore-end -->

## HEAD warn strategy

CLI flag
: `--head-warn-strategy <strategy>`

Environment
: `SAURRON_HEAD_WARN_STRATEGY`

TOML key
: `head_warn_strategy`

Controls whether a warning is logged when a manifest HEAD request to a registry fails. Accepted
values:

| Value    | Behaviour                                                                  |
| -------- | -------------------------------------------------------------------------- |
| `auto`   | Warn only for registries known to support HEAD reliably (Docker Hub, ghcr.io); suppress warnings for all others. This is the default. |
| `always` | Always log a warning on HEAD failure, regardless of registry.              |
| `never`  | Suppress all HEAD failure warnings.                                        |

## Registry username

CLI flag
: `--registry-username <username>`

Environment
: `SAURRON_REGISTRY_USERNAME`

TOML key
: `registry_username`

Global username for authenticating with image registries. When provided alongside
`--registry-password`, credentials are sent as HTTP Basic Auth to the registry's token endpoint to
obtain a scoped Bearer token. Applied to any registry that has no entry in
[`[[registry_credentials]]`](#per-registry-credentials).

## Registry password

CLI flag
: `--registry-password <password>`

Environment
: `SAURRON_REGISTRY_PASSWORD`

TOML key
: `registry_password`

Global password for authenticating with image registries. Applied alongside `--registry-username`
to any registry that has no entry in [`[[registry_credentials]]`](#per-registry-credentials).

This field supports Docker secret file path substitution — if the value is a path to a readable
file, it is replaced with the file contents at startup.

## Per-registry credentials

TOML key
: `[[registry_credentials]]` (array of tables, TOML config file only)

Credential overrides for individual registries. Each entry specifies a `host` and optional
`username` and `password`. Per-registry entries take priority over the global
`registry_username`/`registry_password`. CLI and environment variable configuration is not
supported for this field; use the TOML config file.

**Credential resolution order:**

1. Per-registry entry with `username` and `password` — use those credentials.
2. Per-registry entry with no `username`/`password` — anonymous access (overrides global credentials for that registry).
3. No per-registry entry, global credentials set — use global credentials.
4. No per-registry entry, no global credentials — anonymous access.

**Docker Hub alias:** Both `docker.io` and `registry-1.docker.io` are accepted as the `host` value
for Docker Hub; Saurron normalises them to `registry-1.docker.io` internally.

### Example

```toml
# Authenticated access to GitHub Container Registry
[[registry_credentials]]
host = "ghcr.io"
username = "myuser"
password = "ghp_mytoken"

# Authenticated access to Docker Hub (docker.io alias accepted)
[[registry_credentials]]
host = "docker.io"
username = "hubuser"
password = "hubpass"

# Force anonymous access to quay.io even when global credentials are set
[[registry_credentials]]
host = "quay.io"
```
