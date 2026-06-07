---
layout: page
title: Logging
---

<!-- prettier-ignore-start -->
# Logging
{: .no_toc }

<!-- prettier-ignore-end -->

Logging overview for Saurron.

<!-- prettier-ignore-start -->
* TOC
{:toc}

<!-- prettier-ignore-end -->

## Log level and format

Saurron's log verbosity and output format are controlled with `--log-level` and `--log-format`.
See [Configuration — Logging]({% link _start/config/logging.md %}) for the full reference.

## RUST_LOG

`RUST_LOG` is an environment variable read by Saurron at startup that lets you set log levels on a
per-crate or per-module basis. It uses the same syntax as the
[`tracing-subscriber` `EnvFilter`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html)
and is evaluated in addition to `--log-level` / `SAURRON_LOG_LEVEL`.

### Directive syntax

A `RUST_LOG` value is a comma-separated list of directives. Each directive is one of:

- A bare level (`info`, `debug`, `warn`, `error`, `trace`) — sets the global floor.
- `target=level` — sets the level for a specific crate or module.
- `target[span]=level` — sets the level for events inside a specific span (advanced).

More specific directives take priority over less specific ones. Directives in `RUST_LOG` take
priority over the `--log-level` / `SAURRON_LOG_LEVEL` setting.

### Common examples

Suppress verbose output from HTTP and Docker client libraries while keeping Saurron at debug:

```shell
RUST_LOG=saurron=debug,reqwest=warn,bollard=warn saurron
```

Raise the global level to `warn` but keep Saurron's own update cycle at `info`:

```shell
RUST_LOG=warn,saurron::update=info saurron
```

Show trace-level output only for registry interactions:

```shell
RUST_LOG=saurron=info,saurron::registry=trace saurron
```

### Saurron module targets

| Target | What it covers |
| ------ | -------------- |
| `saurron` | All Saurron events (catch-all) |
| `saurron::update` | Update cycle: freshness checks, pull, restart, rollback |
| `saurron::registry` | Registry HTTP requests and manifest parsing |
| `saurron::docker` | Docker daemon calls |
| `saurron::http` | HTTP API request handling |
| `saurron::scheduler` | Scheduled cycle dispatch |
| `saurron::access` | HTTP access log events (one per request) |
| `saurron::audit` | Audit log events (one per update or rollback) |

Third-party crate targets you may want to quiet: `bollard`, `reqwest`, `hyper`, `tokio`.

