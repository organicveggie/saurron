---
layout: page
title: HTTP API
parent: Configuration
---

<!-- prettier-ignore-start -->
# HTTP API
{: .no_toc }

<!-- prettier-ignore-end -->

Saurron exposes an optional HTTP API for triggering update cycles and scraping Prometheus metrics.
The server starts on a configurable port when any API feature is enabled. All endpoints except
`GET /v1/health` require Bearer token authentication; if a token is not configured when an
authenticated endpoint is enabled, Saurron exits with an error at startup.

`GET /v1/health` is always available and requires no authentication. It returns `200 OK` when
the service is running and is suitable for use as a Docker healthcheck.

<!-- prettier-ignore-start -->
* TOC
{:toc}

<!-- prettier-ignore-end -->

## Enable update endpoint

CLI flag
: `--http-api-update`

Environment
: `SAURRON_HTTP_API_UPDATE`

TOML key
: `http_api.update`

Enable the `POST /v1/update` endpoint, which triggers an immediate update cycle. Supports optional
`?container=<name>` and `?image=<name>` query parameters to restrict the cycle to specific
containers or images.

## Enable metrics endpoint

CLI flag
: `--http-api-metrics`

Environment
: `SAURRON_HTTP_API_METRICS`

TOML key
: `http_api.metrics`

Enable the `GET /v1/metrics` endpoint, which serves Prometheus metrics in standard text exposition
format.

## Token

CLI flag
: `--http-api-token <token>`

Environment
: `SAURRON_HTTP_API_TOKEN`

TOML key
: `http_api.token`

Bearer token required for authenticated API requests. Clients must supply this value in the
`Authorization: Bearer <token>` header. Must be set when any authenticated endpoint is enabled;
Saurron will not start without it.

This field supports Docker secret file path substitution — if the value is a path to a readable
file, it is replaced with the file contents at startup.

## Port

CLI flag
: `--http-api-port <port>`

Environment
: `SAURRON_HTTP_API_PORT`

TOML key
: `http_api.port`

Port the HTTP API server listens on. Default: `8080`.

## Metrics no auth

CLI flag
: `--http-api-metrics-no-auth`

Environment
: `SAURRON_HTTP_API_METRICS_NO_AUTH`

TOML key
: `http_api.metrics_no_auth`

Serve `GET /v1/metrics` without requiring a Bearer token. Useful when Prometheus scrapes from a
trusted network and does not support bearer token authentication. Default: `false`.

## Access log {: #access-log}

CLI flag
: `--http-api-access-log <path>`

Environment
: `SAURRON_HTTP_API_ACCESS_LOG`

TOML key
: `http_api.access_log`

Optional path to an append-only HTTP access log file. Written in newline-delimited structured
JSON. Records one entry per request. Saurron does not write an access log by default.

Logged fields: timestamp, remote address, host, full URI, path, HTTP method, protocol version,
scheme, User-Agent (full value), Authorization type (value redacted), response status code, and
elapsed time in milliseconds.

### Sample entry

```json
{
  "timestamp": "2026-05-05T12:34:56.789Z",
  "level": "INFO",
  "target": "saurron::access",
  "fields": {
    "remote_addr": "192.168.1.100:54321",
    "host": "saurron.example.com:8080",
    "uri": "/v1/update?container=nginx",
    "path": "/v1/update",
    "method": "POST",
    "protocol": "HTTP/1.1",
    "scheme": "http",
    "user_agent": "curl/8.1.2",
    "authorization": "Bearer [REDACTED]",
    "status": 200,
    "duration_ms": 42
  }
}
```
