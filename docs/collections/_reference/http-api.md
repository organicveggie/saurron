---
layout: page
title: HTTP API Reference
---

<!-- prettier-ignore-start -->
# HTTP API
{: .no_toc }

- TOC
{:toc}

<!-- prettier-ignore-end -->

When you run Saurron, the app starts an HTTP server listening on port 8080 (configurable with the
`--http-api-port` option).

All endpoints except `GET /v1/health` require Bearer token auth (`Authorization: Bearer <token>`).
The `GET /v1/metrics` endpoint can be made unauthenticated via `--http-api-metrics-no-auth` - useful
when Prometheus scrapes from trusted network without bearer token support.

<!-- prettier-ignore -->
{: .important}
The HTTP API is not available when using the `--run-once` option.

## GET `/v1/health`

Returns `200 OK` when service running. Suitable as Docker healthcheck. Unauthenticated; no Bearer
token required.

**Example**

```shell
$ curl http://localhost:8080/v1/health
{"status":"ok"}
```

## GET `/v1/metrics`

Returns Prometheus metrics in standard text exposition format. Tracked metrics:

| Metric                       | Type    | Description                                    |
| ---------------------------- | ------- | ---------------------------------------------- |
| `saurron_scans_total`        | Counter | Total update cycles run                        |
| `saurron_scans_skipped`      | Counter | Cycles skipped due to concurrent update        |
| `saurron_containers_scanned` | Gauge   | Containers checked in last cycle               |
| `saurron_containers_updated` | Gauge   | Containers updated in last cycle               |
| `saurron_containers_failed`  | Gauge   | Containers that failed to update in last cycle |

**Response (JSON)**

```json
{
  "saurron_scans_total": 3,
  "saurron_scans_skipped": 0,
  "saurron_containers_scanned": 35,
  "saurron_containers_updated": 7,
  "saurron_containers_failed": 1
}
```

**Example**

```shell
$ curl http://localhost:8080/v1/metrics \
    -H 'Authorization: Bearer ABC123'
```

## POST `/v1/update`

Triggers an immediate update check. By default, Saurron will only check containers in the `running`
state. The following options enable additional states:

| Container state | Option                                                |
| :-------------- | :---------------------------------------------------- |
| `created`       | `--revive-stopped` / `SAURRON_REVIVE_STOPPED`         |
| `exited`        | `--revive-stopped` / `SAURRON_REVIVE_STOPPED`         |
| `restarting`    | `--include-restarting` / `SAURRON_INCLUDE_RESTARTING` |

If Saurron is already in the middle of an update cycle, this will return `409 Conflict` with an
empty response body.

**Parameters**

Scope filters can be supplied as query parameters, as a JSON request body, or as a
form-encoded request body. Supplying the same field in both query params and the body
returns `400 Bad Request`.

| Field | Description |
| :---- | :---------- |
| `container` | Restrict to one or more container names. Comma-separated string or (JSON only) array. |
| `image` | Restrict to containers whose image starts with the given prefix. Comma-separated string or (JSON only) array. |

**Status codes**

| Code | Meaning |
| :--- | :------ |
| `200 OK` | Cycle completed; body is a JSON `SessionReport`. |
| `400 Bad Request` | A filter field was supplied in both query params and body, or the body could not be parsed. Body contains `{"error": "..."}`. |
| `409 Conflict` | Another update cycle is already running. |
| `415 Unsupported Media Type` | A non-empty body was sent with a `Content-Type` other than `application/json` or `application/x-www-form-urlencoded`. |

**Examples**

No filter — update all containers:

```shell
$ curl -XPOST http://localhost:8080/v1/update \
    -H 'Authorization: Bearer ABC123'
```

Query parameter:

```shell
$ curl -XPOST 'http://localhost:8080/v1/update?container=nginx,redis' \
    -H 'Authorization: Bearer ABC123'
```

JSON body — comma-separated string:

```shell
$ curl -XPOST http://localhost:8080/v1/update \
    -H 'Authorization: Bearer ABC123' \
    -H 'Content-Type: application/json' \
    -d '{"container": "nginx,redis"}'
```

JSON body — array:

```shell
$ curl -XPOST http://localhost:8080/v1/update \
    -H 'Authorization: Bearer ABC123' \
    -H 'Content-Type: application/json' \
    -d '{"container": ["nginx", "redis"], "image": "myorg/myapp"}'
```

Form-encoded body:

```shell
$ curl -XPOST http://localhost:8080/v1/update \
    -H 'Authorization: Bearer ABC123' \
    --data-urlencode 'container=nginx,redis'
```
