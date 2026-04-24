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
