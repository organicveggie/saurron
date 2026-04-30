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
OK
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

**Query parameters (Optional)**

- `image=myorg/myapp` - restrict to containers using this image (comma-separated).
- `container=mycontainer` — restrict to specific container by name (comma-separated)

**Response**

**Examples**

```shell
$ curl -XPOST http://localhost:8080/v1/metrics \
    -H 'Authorization: Bearer ABC123' \
```

```shell
$ curl -XPOST http://localhost:8080/v1/metrics?image=myorg/myapp \
    -H 'Authorization: Bearer ABC123' \
```

```shell
$ curl -XPOST http://localhost:8080/v1/metrics?container=mycontainer \
    -H 'Authorization: Bearer ABC123' \
```
