---
layout: page
title: Configuration reference
---

<!-- prettier-ignore-start -->
# Configuration Reference
{: .no_toc }

<!-- prettier-ignore-end -->

All configuration options in alphabetical order by CLI flag. All environment variables use the
prefix `SAURRON_` unless noted otherwise. Options that have no CLI flag, environment variable, or
TOML key equivalent are marked —. TOML-only options (no CLI flag or environment variable) are
listed at the end.

| CLI Flag                                                     | Environment Variable                         | TOML Key                                |
| ------------------------------------------------------------ | -------------------------------------------- | --------------------------------------- |
| `--api-version <version>`                                    | `DOCKER_API_VERSION`                         | `docker.api_version`                    |
| `--audit-log <path>`                                         | `SAURRON_AUDIT_LOG`                          | `audit_log`                             |
| `--cleanup`                                                  | `SAURRON_CLEANUP`                            | `cleanup`                               |
| `--config <path>`                                            | `SAURRON_CONFIG`                             | —                                       |
| `--containers <names>`                                       | `SAURRON_CONTAINERS`                         | `containers`                            |
| `--debug`                                                    | —                                            | —                                       |
| `--disable-containers <names>`                               | `SAURRON_DISABLE_CONTAINERS`                 | `disable_containers`                    |
| `--global-takes-precedence`                                  | `SAURRON_GLOBAL_TAKES_PRECEDENCE`            | `global_takes_precedence`               |
| `--head-warn-strategy <strategy>`                            | `SAURRON_HEAD_WARN_STRATEGY`                 | `head_warn_strategy`                    |
| `--host <uri>`                                               | `DOCKER_HOST`                                | `docker.host`                           |
| `--http-api-metrics`                                         | `SAURRON_HTTP_API_METRICS`                   | `http_api.metrics`                      |
| `--http-api-metrics-no-auth`                                 | `SAURRON_HTTP_API_METRICS_NO_AUTH`           | `http_api.metrics_no_auth`              |
| `--http-api-port <port>`                                     | `SAURRON_HTTP_API_PORT`                      | `http_api.port`                         |
| `--http-api-token <token>`                                   | `SAURRON_HTTP_API_TOKEN`                     | `http_api.token`                        |
| `--http-api-update`                                          | `SAURRON_HTTP_API_UPDATE`                    | `http_api.update`                       |
| `--include-restarting`                                       | `SAURRON_INCLUDE_RESTARTING`                 | `include_restarting`                    |
| `--interval <duration>`                                      | `SAURRON_POLL_INTERVAL`                      | `poll_interval`                         |
| `--label-enable`                                             | `SAURRON_LABEL_ENABLE`                       | `label_enable`                          |
| `--log-access-to-stdout`                                     | `SAURRON_LOG_ACCESS_TO_STDOUT`               | `log_access_to_stdout`                  |
| `--log-audit-to-stdout`                                      | `SAURRON_LOG_AUDIT_TO_STDOUT`                | `log_audit_to_stdout`                   |
| `--log-format <format>`                                      | `SAURRON_LOG_FORMAT`                         | `log_format`                            |
| `--log-level <level>`                                        | `SAURRON_LOG_LEVEL`                          | `log_level`                             |
| `--monitor-only`                                             | `SAURRON_MONITOR_ONLY`                       | `monitor_only`                          |
| `--no-pull`                                                  | `SAURRON_NO_PULL`                            | `no_pull`                               |
| `--notification-delay <duration>`                            | `SAURRON_NOTIFICATION_DELAY`                 | `notifications.general.delay`           |
| `--notification-email-from <address>`                        | `SAURRON_NOTIFICATION_EMAIL_FROM`            | `notifications.email.from`              |
| `--notification-email-password <password>`                   | `SAURRON_NOTIFICATION_EMAIL_PASSWORD`        | `notifications.email.password`          |
| `--notification-email-port <port>`                           | `SAURRON_NOTIFICATION_EMAIL_PORT`            | `notifications.email.port`              |
| `--notification-email-server <host>`                         | `SAURRON_NOTIFICATION_EMAIL_SERVER`          | `notifications.email.server`            |
| `--notification-email-tls-skip-verify`                       | `SAURRON_NOTIFICATION_EMAIL_TLS_SKIP_VERIFY` | `notifications.email.tls_skip_verify`   |
| `--notification-email-to <addresses>`                        | `SAURRON_NOTIFICATION_EMAIL_TO`              | `notifications.email.to`                |
| `--notification-email-user <user>`                           | `SAURRON_NOTIFICATION_EMAIL_USER`            | `notifications.email.user`              |
| `--notification-mqtt-broker <url>`                           | `SAURRON_NOTIFICATION_MQTT_BROKER`           | `notifications.mqtt.broker`             |
| `--notification-mqtt-client-id <id>`                         | `SAURRON_NOTIFICATION_MQTT_CLIENT_ID`        | `notifications.mqtt.client_id`          |
| `--notification-mqtt-password <password>`                    | `SAURRON_NOTIFICATION_MQTT_PASSWORD`         | `notifications.mqtt.password`           |
| `--notification-mqtt-qos <level>`                            | `SAURRON_NOTIFICATION_MQTT_QOS`              | `notifications.mqtt.qos`                |
| `--notification-mqtt-topic <topic>`                          | `SAURRON_NOTIFICATION_MQTT_TOPIC`            | `notifications.mqtt.topic`              |
| `--notification-mqtt-tls-ca-cert <path>`                     | `SAURRON_NOTIFICATION_MQTT_TLS_CA_CERT`      | `notifications.mqtt.tls_ca_cert`        |
| `--notification-mqtt-tls-cert <path>`                        | `SAURRON_NOTIFICATION_MQTT_TLS_CERT`         | `notifications.mqtt.tls_cert`           |
| `--notification-mqtt-tls-key <path>`                         | `SAURRON_NOTIFICATION_MQTT_TLS_KEY`          | `notifications.mqtt.tls_key`            |
| `--notification-mqtt-tls-skip-verify`                        | `SAURRON_NOTIFICATION_MQTT_TLS_SKIP_VERIFY`  | `notifications.mqtt.tls_skip_verify`    |
| `--notification-mqtt-username <user>`                        | `SAURRON_NOTIFICATION_MQTT_USERNAME`         | `notifications.mqtt.username`           |
| `--notification-pushover-token <token>`                      | `SAURRON_NOTIFICATION_PUSHOVER_TOKEN`        | `notifications.pushover.token`          |
| `--notification-pushover-user-key <key>`                     | `SAURRON_NOTIFICATION_PUSHOVER_USER_KEY`     | `notifications.pushover.user_key`       |
| `--notification-template <template>`                         | `SAURRON_NOTIFICATION_TEMPLATE`              | `notifications.general.template`        |
| `--notify-on-every-cycle`                                    | `SAURRON_NOTIFY_ON_EVERY_CYCLE`              | `notifications.general.notify_on_every_cycle` |
| `--registry-password <password>`                             | `SAURRON_REGISTRY_PASSWORD`                  | `registry_password`                     |
| `--registry-username <username>`                             | `SAURRON_REGISTRY_USERNAME`                  | `registry_username`                     |
| `--revive-stopped`                                           | `SAURRON_REVIVE_STOPPED`                     | `revive_stopped`                        |
| `--rollback-on-exit-code` / `--no-rollback-on-exit-code`     | `SAURRON_ROLLBACK_ON_EXIT_CODE`              | `rollback.on_exit_code`                 |
| `--rollback-on-healthcheck` / `--no-rollback-on-healthcheck` | `SAURRON_ROLLBACK_ON_HEALTHCHECK`            | `rollback.on_healthcheck`               |
| `--rollback-on-timeout` / `--no-rollback-on-timeout`         | `SAURRON_ROLLBACK_ON_TIMEOUT`                | `rollback.on_timeout`                   |
| `--run-once`                                                 | `SAURRON_RUN_ONCE`                           | `run_once`                              |
| `--schedule <cron>`                                          | `SAURRON_SCHEDULE`                           | `schedule`                              |
| `--startup-timeout <duration>`                               | `SAURRON_STARTUP_TIMEOUT`                    | `rollback.startup_timeout`              |
| `--stop-timeout <duration>`                                  | `SAURRON_STOP_TIMEOUT`                       | `stop_timeout`                          |
| `--tls-ca-cert <path>`                                       | `DOCKER_CERT_PATH`                           | `docker.tls_ca_cert`                    |
| `--tls-cert <path>`                                          | —                                            | `docker.tls_cert`                       |
| `--tls-key <path>`                                           | —                                            | `docker.tls_key`                        |
| `--tlsverify`                                                | `DOCKER_TLS_VERIFY`                          | `docker.tls_verify`                     |
| `--trace`                                                    | —                                            | —                                       |
| `--webhook-headers <headers>`                                | `SAURRON_WEBHOOK_HEADERS`                    | `notifications.webhook.headers`         |
| `--webhook-tls-skip-verify`                                  | `SAURRON_WEBHOOK_TLS_SKIP_VERIFY`            | `notifications.webhook.tls_skip_verify` |
| `--webhook-url <url>`                                        | `SAURRON_WEBHOOK_URL`                        | `notifications.webhook.url`             |

**TOML-only options** (no CLI flag or environment variable):

| CLI Flag | Environment Variable | TOML Key |
| -------- | -------------------- | -------- |
| — | — | `[[registry_credentials]]` |
