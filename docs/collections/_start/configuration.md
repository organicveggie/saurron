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
flags. Options specified through higher priority sources override options specified through lower
priority sources. Option priority order from highest to lowest is:

1. **CLI flags**
2. **Environment variables**
3. **Config file** (TOML format)
4. **Built-in defaults**

All sources support all options.

See [config reference]({% link _reference/config-reference.md %}) for the complete list of every
configuration option.

# Secret file resolution

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

# Options

## Command line flags

All options on the command line have a long form, which looks like `--long-form-option <value>`.
Some more common options have a single-letter short form as well, which look like `-f <value>`. Some
options take multiple arguments, which are always separated by commands:

```shell
--variadic-option arg1,arg2,arg3
```

Command line flags have the _highest_ priority and override options from any other source.

## Environment variables

With the exception of the standard Docker options, all environment variables start with the prefix
`SAURRON_`. Environment variables have the _second_ highest priority and will override options
specified in the config file.

## Config file

The optional configuration file uses [TOML syntax](https://toml.io/en). Key features of TOML
include:

- Maps unambiguously to a hash table
- Supports inline comments
- Includes native types: KV pairs, arrays, tables, inline tables, arrays of tables, integers,
  floats, bools, dates, and times

The config file path defaults to `/etc/saurron/config.toml`. You can override this via the
`--config` CLI flag or the `SAURRON_CONFIG` environment variable.

To generate a sample config file, use the `--generate-config {path}` command. By default, without
the optional `{path}` parameter, this command will stream the config to standard out. Include the
optional `{path}` parameter to save the generated config using the specified filename.

<!-- prettier-ignore-start -->
### Examples
{: .no_toc }

#### Docker
{: .no_toc }

<!-- prettier-ignore-end -->

```shell
docker run --name saurron \
    ghcr.io/organicveggie/saurron:latest \
    --generate-config /etc/saurron.toml
```

<!-- prettier-ignore-start -->
#### Binary
{: .no_toc }

<!-- prettier-ignore-end -->

```shell
saurron --generate-config
```

## Details

### Docker

| Purpose                                                                  | CLI Flag                  | Environment Variable | TOML Key             |
| :----------------------------------------------------------------------- | :------------------------ | :------------------- | :------------------- |
| Docker daemon socket or host URL. Default: `unix:///var/run/docker.sock` | `--host <uri>`            | `DOCKER_HOST`        | `docker.host`        |
| Enable TLS for Docker daemon connection                                  | `--tlsverify`             | `DOCKER_TLS_VERIFY`  | `docker.tls_verify`  |
| Path to TLS CA certificate                                               | `--tls-ca-cert <path>`    | `DOCKER_CERT_PATH`   | `docker.tls_ca_cert` |
| Path to TLS client certificate                                           | `--tls-cert <path>`       | —                    | `docker.tls_cert`    |
| Path to TLS client key                                                   | `--tls-key <path>`        | —                    | `docker.tls_key`     |
| Docker API version to negotiate. Default: auto-negotiate                 | `--api-version <version>` | `DOCKER_API_VERSION` | `docker.api_version` |
