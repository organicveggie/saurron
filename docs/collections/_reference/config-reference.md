---
layout: page
title: Configuration reference
---

# Table of contents

* TOC
{:toc}

{: .note}
All environment variables use the prefix `SAURRON_`.

# Container Selection

| Purpose                                                                                                             | CLI Flag                       | Environment Variable              | TOML Key                  |
| :------------------------------------------------------------------------------------------------------------------ | :----------------------------- | :-------------------------------- | :------------------------ |
| Opt-in mode: only update containers with `saurron.enable=true`                                                      | `--label-enable`               | `SAURRON_LABEL_ENABLE`            | `label_enable`            |
| Comma-separated container names to always exclude                                                                   | `--disable-containers <names>` | `SAURRON_DISABLE_CONTAINERS`      | `disable_containers`      |
| Comma-separated container names allow-list; all others are ignored                                                  | `--containers <names>`         | `SAURRON_CONTAINERS`              | `containers`              |
| Include containers in `restarting` state                                                                            | `--include-restarting`         | `SAURRON_INCLUDE_RESTARTING`      | `include_restarting`      |
| Global flags take precedence over per-container labels for `monitor-only` and `no-pull` (default: label precedence) | `--global-takes-precedence`    | `SAURRON_GLOBAL_TAKES_PRECEDENCE` | `global_takes_precedence` |

# Docker Connection

| Purpose                                                                  | CLI Flag                  | Environment Variable | TOML Key             |
| :----------------------------------------------------------------------- | :------------------------ | :------------------- | :------------------- |
| Docker daemon socket or host URL. Default: `unix:///var/run/docker.sock` | `--host <uri>`            | `DOCKER_HOST`        | `docker.host`        |
| Enable TLS for Docker daemon connection                                  | `--tlsverify`             | `DOCKER_TLS_VERIFY`  | `docker.tls_verify`  |
| Path to TLS CA certificate                                               | `--tls-ca-cert <path>`    | `DOCKER_CERT_PATH`   | `docker.tls_ca_cert` |
| Path to TLS client certificate                                           | `--tls-cert <path>`       | —                    | `docker.tls_cert`    |
| Path to TLS client key                                                   | `--tls-key <path>`        | —                    | `docker.tls_key`     |
| Docker API version to negotiate. Default: auto-negotiate                 | `--api-version <version>` | `DOCKER_API_VERSION` | `docker.api_version` |

# General

| Purpose                                                                | CLI Flag                | Environment Variable | TOML Key           |
| :--------------------------------------------------------------------- | :---------------------- | :------------------- | :----------------- |
| Path to TOML config file                                               | `--config <path>`       | `SAURRON_CONFIG`     | _(not applicable)_ |
| Log level (`trace`, `debug`, `info`, `warn`, `error`). Default: `info` | `--log-level <level>`   | `SAURRON_LOG_LEVEL`  | `log_level`        |
| Log format (`auto`, `json`, `logfmt`, `pretty`). Default: `auto`       | `--log-format <format>` | `SAURRON_LOG_FORMAT` | `log_format`       |
| Shorthand for `--log-level debug`                                      | `--debug`               | —                    | —                  |
| Shorthand for `--log-level trace`                                      | `--trace`               | —                    | —                  |
| Path to append-only audit log file                                     | `--audit-log <path>`    | `SAURRON_AUDIT_LOG`  | `audit_log`        |

# HTTP API

| Purpose                                                        | CLI Flag                     | Environment Variable               | TOML Key                   |
| :------------------------------------------------------------- | :--------------------------- | :--------------------------------- | :------------------------- |
| Enable `POST /v1/update`                                       | `--http-api-update`          | `SAURRON_HTTP_API_UPDATE`          | `http_api.update`          |
| Enable `GET /v1/metrics`                                       | `--http-api-metrics`         | `SAURRON_HTTP_API_METRICS`         | `http_api.metrics`         |
| Bearer token for all API requests                              | `--http-api-token <token>`   | `SAURRON_HTTP_API_TOKEN`           | `http_api.token`           |
| HTTP API server port. Default: `8080`                          | `--http-api-port <port>`     | `SAURRON_HTTP_API_PORT`            | `http_api.port`            |
| Serve `GET /v1/metrics` without Bearer token. Default: `false` | `--http-api-metrics-no-auth` | `SAURRON_HTTP_API_METRICS_NO_AUTH` | `http_api.metrics_no_auth` |

# Notifications

## General

| Purpose                                                                               | CLI Flag                             | Environment Variable            | TOML Key                         |
| :------------------------------------------------------------------------------------ | :----------------------------------- | :------------------------------ | :------------------------------- |
| Delay between cycle completion and notification dispatch (e.g., `30s`). Default: `0s` | `--notification-delay <duration>`    | `SAURRON_NOTIFICATION_DELAY`    | `notifications.general.delay`    |
| Custom notification template string; uses built-in default when omitted               | `--notification-template <template>` | `SAURRON_NOTIFICATION_TEMPLATE` | `notifications.general.template` |

## Webhook

| Purpose                                                                                  | CLI Flag                      | Environment Variable              | TOML Key                                |
| :--------------------------------------------------------------------------------------- | :---------------------------- | :-------------------------------- | :-------------------------------------- |
| URL to POST notification payloads to                                                     | `--webhook-url <url>`         | `SAURRON_WEBHOOK_URL`             | `notifications.webhook.url`             |
| Additional HTTP headers as comma-separated `Key:Value` pairs                             | `--webhook-headers <headers>` | `SAURRON_WEBHOOK_HEADERS`         | `notifications.webhook.headers`         |
| Skip TLS cert verification. Default: `false` — invalid cert logs error and skips webhook | `--webhook-tls-skip-verify`   | `SAURRON_WEBHOOK_TLS_SKIP_VERIFY` | `notifications.webhook.tls_skip_verify` |

## Email

| Purpose                                | CLI Flag                                   | Environment Variable                         | TOML Key                              |
| :------------------------------------- | :----------------------------------------- | :------------------------------------------- | :------------------------------------ |
| Sender address                         | `--notification-email-from <address>`      | `SAURRON_NOTIFICATION_EMAIL_FROM`            | `notifications.email.from`            |
| Recipient address(es), comma-separated | `--notification-email-to <addresses>`      | `SAURRON_NOTIFICATION_EMAIL_TO`              | `notifications.email.to`              |
| SMTP server hostname                   | `--notification-email-server <host>`       | `SAURRON_NOTIFICATION_EMAIL_SERVER`          | `notifications.email.server`          |
| SMTP server port. Default: `587`       | `--notification-email-port <port>`         | `SAURRON_NOTIFICATION_EMAIL_PORT`            | `notifications.email.port`            |
| SMTP auth username                     | `--notification-email-user <user>`         | `SAURRON_NOTIFICATION_EMAIL_USER`            | `notifications.email.user`            |
| SMTP auth password                     | `--notification-email-password <password>` | `SAURRON_NOTIFICATION_EMAIL_PASSWORD`        | `notifications.email.password`        |
| Skip TLS cert verification for SMTP    | `--notification-email-tls-skip-verify`     | `SAURRON_NOTIFICATION_EMAIL_TLS_SKIP_VERIFY` | `notifications.email.tls_skip_verify` |

## MQTT

| Purpose                                                                                    | CLI Flag                                  | Environment Variable                  | TOML Key                       |
| :----------------------------------------------------------------------------------------- | :---------------------------------------- | :------------------------------------ | :----------------------------- |
| MQTT broker URL (e.g., `tcp://broker.example.com:1883` or `ssl://broker.example.com:8883`) | `--notification-mqtt-broker <url>`        | `SAURRON_NOTIFICATION_MQTT_BROKER`    | `notifications.mqtt.broker`    |
| MQTT topic for notifications                                                               | `--notification-mqtt-topic <topic>`       | `SAURRON_NOTIFICATION_MQTT_TOPIC`     | `notifications.mqtt.topic`     |
| MQTT QoS: `0` (at most once), `1` (at least once), `2` (exactly once). Default: `0`        | `--notification-mqtt-qos <level>`         | `SAURRON_NOTIFICATION_MQTT_QOS`       | `notifications.mqtt.qos`       |
| MQTT client ID; auto-generated if omitted                                                  | `--notification-mqtt-client-id <id>`      | `SAURRON_NOTIFICATION_MQTT_CLIENT_ID` | `notifications.mqtt.client_id` |
| MQTT broker auth username                                                                  | `--notification-mqtt-username <user>`     | `SAURRON_NOTIFICATION_MQTT_USERNAME`  | `notifications.mqtt.username`  |
| MQTT broker auth password                                                                  | `--notification-mqtt-password <password>` | `SAURRON_NOTIFICATION_MQTT_PASSWORD`  | `notifications.mqtt.password`  |

## Pushover

| Purpose                        | CLI Flag                                 | Environment Variable                     | TOML Key                          |
| :----------------------------- | :--------------------------------------- | :--------------------------------------- | :-------------------------------- |
| Pushover application API token | `--notification-pushover-token <token>`  | `SAURRON_NOTIFICATION_PUSHOVER_TOKEN`    | `notifications.pushover.token`    |
| Pushover user or group key     | `--notification-pushover-user-key <key>` | `SAURRON_NOTIFICATION_PUSHOVER_USER_KEY` | `notifications.pushover.user_key` |

# Registry

| Purpose                                                                                                                | CLI Flag                          | Environment Variable         | TOML Key             |
| :--------------------------------------------------------------------------------------------------------------------- | :-------------------------------- | :--------------------------- | :------------------- |
| Warning behaviour for failed HEAD requests: `auto` (default — warn only for Docker Hub and ghcr.io), `always`, `never` | `--head-warn-strategy <strategy>` | `SAURRON_HEAD_WARN_STRATEGY` | `head_warn_strategy` |
| Username for registry authentication (applied to all registries)                                                       | `--registry-username <username>`  | `SAURRON_REGISTRY_USERNAME`  | `registry_username`  |
| Password for registry authentication; supports Docker secret file path                                                 | `--registry-password <password>`  | `SAURRON_REGISTRY_PASSWORD`  | `registry_password`  |

# Rollback

| Purpose                                                                                   | CLI Flag                                                     | Environment Variable              | TOML Key                   |
| :---------------------------------------------------------------------------------------- | :----------------------------------------------------------- | :-------------------------------- | :------------------------- |
| Rollback if new container exits non-zero. Default: enabled                                | `--rollback-on-exit-code` / `--no-rollback-on-exit-code`     | `SAURRON_ROLLBACK_ON_EXIT_CODE`   | `rollback.on_exit_code`    |
| Rollback if Docker healthcheck reports unhealthy within startup timeout. Default: enabled | `--rollback-on-healthcheck` / `--no-rollback-on-healthcheck` | `SAURRON_ROLLBACK_ON_HEALTHCHECK` | `rollback.on_healthcheck`  |
| Rollback if container doesn't reach `running` within startup timeout. Default: enabled    | `--rollback-on-timeout` / `--no-rollback-on-timeout`         | `SAURRON_ROLLBACK_ON_TIMEOUT`     | `rollback.on_timeout`      |
| Wait time before triggering rollback. Default: `30s`                                      | `--startup-timeout <duration>`                               | `SAURRON_STARTUP_TIMEOUT`         | `rollback.startup_timeout` |

# Scheduling

| Purpose                                                                                                                          | CLI Flag                | Environment Variable    | TOML Key        |
| :------------------------------------------------------------------------------------------------------------------------------- | :---------------------- | :---------------------- | :-------------- |
| Poll interval as duration (e.g., `5m`, `1h`). Converted to cron internally. Mutually exclusive with `--schedule`. Default: `24h` | `--interval <duration>` | `SAURRON_POLL_INTERVAL` | `poll_interval` |
| Poll schedule as cron expression (e.g., `0 4 * * *`). Mutually exclusive with `--interval`                                       | `--schedule <cron>`     | `SAURRON_SCHEDULE`      | `schedule`      |
| Single update cycle then exit. Mutually exclusive with `--interval` and `--schedule`                                             | `--run-once`            | `SAURRON_RUN_ONCE`      | `run_once`      |

# Update Strategy

| Purpose                                                    | CLI Flag                    | Environment Variable     | TOML Key         |
| :--------------------------------------------------------- | :-------------------------- | :----------------------- | :--------------- |
| Detect + notify; no pull or restart                        | `--monitor-only`            | `SAURRON_MONITOR_ONLY`   | `monitor_only`   |
| Restart using cached image without pulling                 | `--no-pull`                 | `SAURRON_NO_PULL`        | `no_pull`        |
| Remove old images after successful update                  | `--cleanup`                 | `SAURRON_CLEANUP`        | `cleanup`        |
| Start stopped containers after image update                | `--revive-stopped`          | `SAURRON_REVIVE_STOPPED` | `revive_stopped` |
| Wait time for graceful stop before SIGKILL. Default: `10s` | `--stop-timeout <duration>` | `SAURRON_STOP_TIMEOUT`   | `stop_timeout`   |
