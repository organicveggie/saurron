---
layout: page
title: Configuration
---

<!-- prettier-ignore-start -->
# Saurron Configuration
{: .no_toc }

* TOC
{:toc}

<!-- prettier-ignore-end -->

# Overview

Saurron uses a layered configuration that can include TOML file, environment variables, and CLI
flags. Options specified through higher precedence sources override options specified through lower
precedence sources. Precedence order (from highest to lowest):

1. **CLI flags**
2. **Environment variables**
3. **Config file** (TOML format)
4. **Built-in defaults**

All sources support all options. The config file path defaults to `/etc/saurron/config.toml`. You
can override this via the `--config` CLI flag or the `SAURRON_CONFIG` environment variable.

## Secret File Resolution

For a subset of configuration options, if the value of the option is a path to a readable file,
Saurron transparently replaces the value with the file contents at startup. This enables Docker
secrets without embedding sensitive values in env vars or CLI args.

Options supporting file contents substitution:

- `http_api.token`
- `notifications.email.from`
- `notifications.email.password`
- `notifications.email.port`
- `notifications.email.server`
- `notifications.email.to`
- `notifications.email.user`
- `notifications.general.template`
- `notifications.mqtt.broker`
- `notifications.mqtt.client_id`
- `notifications.mqtt.password`
- `notifications.mqtt.topic`
- `notifications.mqtt.username`
- `notifications.webhook.headers`
- `notifications.pushover.token`
- `notifications.pushover.user_key`
- `notifications.webhook.url`
- `registry_password`

## See also

See [config reference]({% link _reference/config-reference.md %}) for the complete list of every
configuration option.
