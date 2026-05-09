# ToDos

## Notifications

- **Configurable notification trigger**: Currently notifications fire only when a cycle produces at least one update, failure, or rollback (`notifications::should_notify` in `src/notifications.rs`). Add a config option (e.g. `notify_on_every_cycle = true`) so operators can receive a notification after every cycle regardless of outcome.

## Miscellaneous

* Dependent container restarts. Restart containers sharing networks or volumes with updated container.
* Docker Hub inbound webhook format. Parse Docker Hub-specific webhook payloads.
* Web UI
    * Dashboard for update history
    * Manual triggers
    * Custom template previews with synthetic data
    * Manually send test notifications
      * Email
      * Pushover
      * MQTT
      * Webhook
* Lifecycle hooks. Pre/post-check and pre/post-update shell commands inside containers; EX_TEMPFAIL exit code to signal skip-without-failure
* Notification template preview. Validate custom templates against synthetic data without real update cycle.
* Scope-based multi-instance support. Multiple instances on same host managing non-overlapping container sets via scope label.
* Multiple instance detection. Detect duplicate instances sharing same scope; stop all but most recently created.
* Docker secrets
* Log to file?
* Third-party authorization / authentication
* u - only on failures
* Use Accept-Encoding header to determine response content types for HTTP API. Current 409 and 401 responses return bare status codes with no body.
* Make Docker Hub retry cap configurable instead of hardcoded as 3.
* Clean up unused images
