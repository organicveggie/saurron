---
layout: page
title: Logging
parent: Configuration
---

<!-- prettier-ignore-start -->
# Logging Options
{: .no_toc }

<!-- prettier-ignore-end -->

Logging settings for Saurron.

<!-- prettier-ignore-start -->
* TOC
{:toc}

<!-- prettier-ignore-end -->

## Log level

CLI flag
: `--log-level <level>`

Environment
: `SAURRON_LOG_LEVEL`

TOML key
: `log_level`

Log verbosity level. Possible values include: `trace`, `debug`, `info`, `warn`, and `error`. Default: `info`.

### Shorthand

`--debug`
: Shorthand for `--log-level debug`

`--trace`
: Shorthand for `--log-level trace`

## Log format

CLI flag
: `--log-format <format>`

Environment
: `SAURRON_LOG_FORMAT`

TOML key
: `log_format`

Format for logging output. Possible values include:

`auto`
: Uses the `pretty` format when standard out is a terminal/tty. Otherwise uses the `json` format.

`json`
: Newline-delimited structured JSON logs. The JSON output is not optimized for human readability.

`logfmt`
: Outputs the structured, key/value logging format used by Heroku and Logplex.

`pretty`
: Excessively pretty, multi-line logs, optimized for human readability.

Default: `auto`.

### Sample output

#### JSON

```json
{"timestamp":"2022-02-15T18:47:10.821315Z","level":"INFO","fields":{"message":"preparing to shave yaks","number_of_yaks":3},"target":"fmt_json"}
{"timestamp":"2022-02-15T18:47:10.821422Z","level":"INFO","fields":{"message":"shaving yaks"},"target":"fmt_json::yak_shave","spans":[{"yaks":3,"name":"shaving_yaks"}]}
```

#### logfmt

```
level=INFO timestamp="2022-02-15T18:47:10.821315Z" message="preparing to shave yaks" number_of_yaks=3 target="logfmt"
level=INFO timestamp="2022-02-15T18:47:10.821422Z" messsage="shaving yaks" yaks=3 name="shaving_yaks" target="logfmt::yak_shave"
```

#### pretty

```
2022-02-15T18:44:24.535324Z  INFO fmt_pretty: preparing to shave yaks, number_of_yaks: 3
  at examples/examples/fmt-pretty.rs:16 on main

2022-02-15T18:44:24.535403Z  INFO fmt_pretty::yak_shave: shaving yaks
  at examples/examples/fmt/yak_shave.rs:41 on main
  in fmt_pretty::yak_shave::shaving_yaks with yaks: 3
```

## Audit log

CLI flag
: `--audit-log <path>`

Environment
: `SAURRON_AUDIT_LOG`

TOML key
: `audit_log`

Optional path to an append-only audit log file. Written in newline-delimited structured JSON. Records every update and rollback. Saurron does not create an audit log by default.

### Sample output

```
{
  "event": "update",
  "container_name": "saurron",
  "container_id": "8779428d56c14798c913e913173b0f4ef232b0d1000267340004986ae74a4152",
  "old_image_tag": "latest",
  "old_image_digest": "sha256:f8314084d6b06b7bae4179a9dbf172bfe3d3dcb0f5cb05ec0ca2e92e185e1b73",
  "new_image_tag": "latest",
  "new_image_digest": "sha256:e72473a766d89a0239cb18b19103bd6b54b5dfbcbe984dbf1af257074eeececf",
  "outcome": "success"
}
{
  "event": "rollback",
  "container_name": "portainer",
  "container_id": "c9671c703c4d3aa374de2ac3a7a74b2644faa04c5cb93d56a5c7368bfe71ba49",
  "attempted_image_tag": "latest",
  "attempted_image_digest": "sha256:2ad8d683056c1d22f18074e123881c257e64d127b9bb34a08f03d5481c53ff70",
  "restored_image_tag": "latest",
  "restored_image_digest": "sha256:2421c752e9ca19bf2155e94e46c3ed83f03a3e601a473f88771c2d8a5b59ab48",
  "outcome": "rollback",
  "failure_reason": "healthcheck_failed"
}
```

## HTTP access log

CLI flag
: `--http-api-access-log <path>`

Environment
: `SAURRON_HTTP_API_ACCESS_LOG`

TOML key
: `http_api.access_log`

The HTTP API access log is configured under the `[http_api]` TOML section and records one
structured JSON entry per incoming request.

See [HTTP API — Access log]({% link start/config/http-api.md %}#access-log) for configuration details.
