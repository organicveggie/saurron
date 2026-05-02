---
layout: page
title: Notifications
parent: Configuration
---

<!-- prettier-ignore-start -->
# Notifications
{: .no_toc }

<!-- prettier-ignore-end -->

Saurron can send notifications to one or more targets after each update cycle. All events from a
single scan are accumulated and delivered together when the cycle completes — not one per container.
Notifications are suppressed if the cycle produced no updates, no stale detections, and no failures.

Multiple notification targets can be active simultaneously.

<!-- prettier-ignore-start -->
* TOC
{:toc}

<!-- prettier-ignore-end -->

## General

### Notification delay

CLI flag
: `--notification-delay <duration>`

Environment
: `SAURRON_NOTIFICATION_DELAY`

TOML key
: `notifications.general.delay`

Delay between cycle completion and notification dispatch (e.g. `30s`). Useful for rate-limiting or
debouncing when multiple targets are configured. Default: `0s`.

### Notification template

CLI flag
: `--notification-template <template>`

Environment
: `SAURRON_NOTIFICATION_TEMPLATE`

TOML key
: `notifications.general.template`

Custom notification template string. Uses [MiniJinja](https://github.com/mitsuhiko/minijinja)
(Jinja2-compatible) syntax. When omitted, the built-in default template is used.

The template context exposes the following fields:

- `timestamp` — ISO 8601 timestamp of the cycle
- `hostname` — Docker host name
- `summary.scanned` — number of containers checked
- `summary.updated` — number of containers updated
- `summary.stale_detected` — number of containers detected stale in monitor-only mode
- `summary.failed` — number of containers that failed to update
- `containers` — list of container results, each with `name`, `old_image`, `new_image`, and `outcome`

`outcome` values:

| Value            | Description                                                         |
| ---------------- | ------------------------------------------------------------------- |
| `updated`        | Image pulled, container restarted successfully                      |
| `stale_detected` | Update detected in monitor-only mode; no restart                    |
| `rolled_back`    | Container started but failed health checks; previous image restored |
| `failed`         | Update attempted but failed; rollback not possible                  |

The built-in default template:

```jinja
Saurron update report — <timestamp>
Host: <hostname>

Summary:
  - <total_scanned> scanned
  - <total_updated> updated
  - <total_stale_detected> stale detected
  - <total_failed> failed

Containers:
  <container-name-1>
    old: <old_image>
    new: <new_image>
    outcome: <outcome>
```

This field supports Docker secret file path substitution — if the value is a path to a readable
file, it is replaced with the file contents at startup.

## Email

Sends notifications via SMTP. Uses STARTTLS by default.

### From address

CLI flag
: `--notification-email-from <address>`

Environment
: `SAURRON_NOTIFICATION_EMAIL_FROM`

TOML key
: `notifications.email.from`

Sender email address.

This field supports Docker secret file path substitution.

### To address

CLI flag
: `--notification-email-to <addresses>`

Environment
: `SAURRON_NOTIFICATION_EMAIL_TO`

TOML key
: `notifications.email.to`

Recipient email address or addresses (comma-separated).

This field supports Docker secret file path substitution.

### SMTP server

CLI flag
: `--notification-email-server <host>`

Environment
: `SAURRON_NOTIFICATION_EMAIL_SERVER`

TOML key
: `notifications.email.server`

SMTP server hostname.

This field supports Docker secret file path substitution.

### SMTP port

CLI flag
: `--notification-email-port <port>`

Environment
: `SAURRON_NOTIFICATION_EMAIL_PORT`

TOML key
: `notifications.email.port`

SMTP server port. Default: `587`.

This field supports Docker secret file path substitution.

### SMTP username

CLI flag
: `--notification-email-user <user>`

Environment
: `SAURRON_NOTIFICATION_EMAIL_USER`

TOML key
: `notifications.email.user`

SMTP authentication username.

This field supports Docker secret file path substitution.

### SMTP password

CLI flag
: `--notification-email-password <password>`

Environment
: `SAURRON_NOTIFICATION_EMAIL_PASSWORD`

TOML key
: `notifications.email.password`

SMTP authentication password.

This field supports Docker secret file path substitution.

### Email TLS skip verify

CLI flag
: `--notification-email-tls-skip-verify`

Environment
: `SAURRON_NOTIFICATION_EMAIL_TLS_SKIP_VERIFY`

TOML key
: `notifications.email.tls_skip_verify`

Skip TLS certificate verification for the SMTP connection. Default: `false`.

## MQTT

Publishes notifications to an MQTT broker topic.

### Broker URL

CLI flag
: `--notification-mqtt-broker <url>`

Environment
: `SAURRON_NOTIFICATION_MQTT_BROKER`

TOML key
: `notifications.mqtt.broker`

MQTT broker URL. Use `tcp://` for unencrypted connections or `ssl://` for TLS (e.g.
`tcp://broker.example.com:1883` or `ssl://broker.example.com:8883`). Setting this field enables
MQTT notifications.

This field supports Docker secret file path substitution.

### Topic

CLI flag
: `--notification-mqtt-topic <topic>`

Environment
: `SAURRON_NOTIFICATION_MQTT_TOPIC`

TOML key
: `notifications.mqtt.topic`

MQTT topic to publish notifications to.

This field supports Docker secret file path substitution.

### QoS level

CLI flag
: `--notification-mqtt-qos <level>`

Environment
: `SAURRON_NOTIFICATION_MQTT_QOS`

TOML key
: `notifications.mqtt.qos`

MQTT quality of service level. Default: `0`.

| Value | Meaning         |
| ----- | --------------- |
| `0`   | At most once    |
| `1`   | At least once   |
| `2`   | Exactly once    |

### Client ID

CLI flag
: `--notification-mqtt-client-id <id>`

Environment
: `SAURRON_NOTIFICATION_MQTT_CLIENT_ID`

TOML key
: `notifications.mqtt.client_id`

MQTT client identifier. Auto-generated if omitted.

This field supports Docker secret file path substitution.

### MQTT username

CLI flag
: `--notification-mqtt-username <user>`

Environment
: `SAURRON_NOTIFICATION_MQTT_USERNAME`

TOML key
: `notifications.mqtt.username`

MQTT broker authentication username.

This field supports Docker secret file path substitution.

### MQTT password

CLI flag
: `--notification-mqtt-password <password>`

Environment
: `SAURRON_NOTIFICATION_MQTT_PASSWORD`

TOML key
: `notifications.mqtt.password`

MQTT broker authentication password.

This field supports Docker secret file path substitution.

## Pushover

Sends real-time push notifications via [Pushover](https://pushover.net/) to Android, iPhone, iPad,
and desktop devices.

### Application token

CLI flag
: `--notification-pushover-token <token>`

Environment
: `SAURRON_NOTIFICATION_PUSHOVER_TOKEN`

TOML key
: `notifications.pushover.token`

Pushover application API token. Setting this field (along with the user key) enables Pushover
notifications.

This field supports Docker secret file path substitution.

### User key

CLI flag
: `--notification-pushover-user-key <key>`

Environment
: `SAURRON_NOTIFICATION_PUSHOVER_USER_KEY`

TOML key
: `notifications.pushover.user_key`

Pushover user or group key.

This field supports Docker secret file path substitution.

## Webhook

Sends a JSON `POST` request to a configurable URL at the end of each update cycle.

### Webhook URL

CLI flag
: `--webhook-url <url>`

Environment
: `SAURRON_WEBHOOK_URL`

TOML key
: `notifications.webhook.url`

URL to `POST` notification payloads to. Setting this field enables webhook notifications.

This field supports Docker secret file path substitution.

### Webhook headers

CLI flag
: `--webhook-headers <headers>`

Environment
: `SAURRON_WEBHOOK_HEADERS`

TOML key
: `notifications.webhook.headers`

Additional HTTP headers to include in the webhook request, as comma-separated `Key:Value` pairs
(e.g. `Authorization:Bearer mytoken,X-Custom:value`).

This field supports Docker secret file path substitution.

### Webhook TLS skip verify

CLI flag
: `--webhook-tls-skip-verify`

Environment
: `SAURRON_WEBHOOK_TLS_SKIP_VERIFY`

TOML key
: `notifications.webhook.tls_skip_verify`

Skip TLS certificate verification for the webhook endpoint. Default: `false`. When disabled, an
invalid certificate logs an error and the webhook delivery is skipped.
