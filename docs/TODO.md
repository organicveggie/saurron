# ToDos

## Notifications

- Configurable notification trigger: Currently notifications fire only when a cycle produces at least one update, failure, or rollback (`notifications::should_notify` in `src/notifications.rs`). Add a config option (e.g. `notify_on_every_cycle = true`) so operators can receive a notification after every cycle regardless of outcome.
- Notification template preview. Validate custom templates against synthetic data without real update cycle.

## Logging

- Log to file in addition to stdout
- Log cleanup (rotation / truncation / removal)

## Multiple instances

- Scope-based multi-instance support. Multiple instances on same host managing non-overlapping container sets via scope label.
- Multiple instance detection. Detect duplicate instances sharing same scope; stop all but most recently created.

## Miscellaneous

- Dependent container restarts. Restart containers sharing networks or volumes with updated container.
- Docker Hub inbound webhook format. Parse Docker Hub-specific webhook payloads.
- Web UI
  - Dashboard for update history
  - Manual triggers
  - Custom template previews with synthetic data
  - Manually send test notifications
    - Email
    - Pushover
    - MQTT
    - Webhook
- Lifecycle hooks. Pre/post-check and pre/post-update shell commands inside containers; EX_TEMPFAIL exit code to signal skip-without-failure
- Docker secrets
- Third-party authorization / authentication
- Use Accept-Encoding header to determine response content types for HTTP API. Current 409 and 401 responses return bare status codes with no body.
- Make Docker Hub retry cap configurable instead of hardcoded as 3.
- Clean up unused images
- Offer alpine based container images
- Proper integration tests — trait abstraction
  - Extract a DockerApi trait from DockerClient, make AppStateInner generic over it (or use Arc<dyn DockerApi>), implement a FakeDockerClient in tests. Full coverage for get_containers. Touches docker.rs, update.rs, http.rs, main.rs — non-trivial refactor, probably 100–200 lines of churn across 4 files.
- CI never builds/tests the web feature

## UI

- CycleStatusCard — running state data
  - The running card in the design shows scanned, total, and current container name. The /v1/health endpoint only has updating: bool — no progress data.

### Out of scope for v1

- **Other panels** - Manual Update, Template Preview, Test Notifications pages
- **Authentication** — Enforce existing `http_api.token` Bearer token on `/ui/*` and five new API endpoints. Add login page prompting for token, stores in `sessionStorage`. Add `401` redirect handling in Svelte fetch wrapper.
- **History search and filtering** — Date range picker, outcome filter, container name search on dashboard table.
- **Live cycle progress** — Replace 5-second health poll with Server-Sent Events for real-time container-by-container progress during running cycle.
- **Notification target configuration UI** — View and edit notification settings from browser rather than editing config files directly.
- Accent color picker (Ember, Teal, Indigo accents)
- Server-side history search / filtering
- Cleanup incomplete cycles in `cycles` table
- Show count/list of _disabled_ containers excluded from processing
