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
Registry HTTP API v2 to fetch manifests and compare digests without pulling images. A single set of
credentials is applied to all registries; per-registry credential scoping is a future enhancement.

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

Username for authenticating with image registries. When provided alongside `--registry-password`,
credentials are sent as HTTP Basic Auth to the registry's token endpoint to obtain a scoped Bearer
token. Applied to all registries.

## Registry password

CLI flag
: `--registry-password <password>`

Environment
: `SAURRON_REGISTRY_PASSWORD`

TOML key
: `registry_password`

Password for authenticating with image registries. Applied to all registries alongside
`--registry-username`.

This field supports Docker secret file path substitution — if the value is a path to a readable
file, it is replaced with the file contents at startup.
