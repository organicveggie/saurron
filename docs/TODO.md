# ToDos

## Notifications

- **Configurable notification trigger**: Currently notifications fire only when a cycle produces at least one update, failure, or rollback (`notifications::should_notify` in `src/notifications.rs`). Add a config option (e.g. `notify_on_every_cycle = true`) so operators can receive a notification after every cycle regardless of outcome.

- **MQTT TLS support**: `send_mqtt` in `src/notifications.rs` connects with plain TCP only. The `MqttConfig` struct has no TLS fields. Add `tls_verify`, `tls_ca_cert`, `tls_cert`, `tls_key` and wire them into the `MqttOptions` builder.

## Docker Hub Rate Limiting

https://docs.docker.com/reference/api/hub/latest/#tag/rate-limiting

If you haven't hit the limit, each request to the API will return the following headers in the response.

- `X-RateLimit-Limit` - The limit of requests per minute.
- `X-RateLimit-Remaining` - The remaining amount of calls within the limit period.
- `X-RateLimit-Reset` - The unix timestamp of when the remaining resets.

If you have hit the limit, you will receive a response status of `429` and the `Retry-After` header in the response.

The `Retry-After` header specifies the number of seconds to wait until you can call the API again.

## Miscellaneous

* Per-registry credential scoping. Separate username/password per registry; Docker config file credential source.
* Dependent container restarts. Restart containers sharing networks or volumes with updated container.
* Docker Hub inbound webhook format. Parse Docker Hub-specific webhook payloads.
* Web UI: Dashboard for update history and manual triggers.
* Lifecycle hooks. Pre/post-check and pre/post-update shell commands inside containers; EX_TEMPFAIL exit code to signal skip-without-failure
* Notification template preview. Validate custom templates against synthetic data without real update cycle.
* Scope-based multi-instance support. Multiple instances on same host managing non-overlapping container sets via scope label
* Multiple instance detection. Detect duplicate instances sharing same scope; stop all but most recently created.
* HTTP API: Update `POST /v1/update` to support embedding the request parameters in the request body with either `application/json` or `application/x-www-form-urlencoded` content types.
