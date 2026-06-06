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

## UI

- CycleStatusCard — "Next cycle" data source
  - The plan says to show "Next cycle countdown, schedule interval, watched count." But the /v1/health endpoint only returns { updating, version, hostname }. There's no schedule interval, next-cycle ETA, or watched count. Currently using placeholder values.
- CycleStatusCard — running state data
  - The running card in the design shows scanned, total, and current container name. The /v1/health endpoint only has updating: bool — no progress data.

## Version updates

### Rust

- sqlx 0.8 → 0.9
  - [Changelog](https://github.com/transact-rs/sqlx/blob/main/CHANGELOG.md) ([raw](https://raw.githubusercontent.com/transact-rs/sqlx/refs/heads/main/CHANGELOG.md))
  - Relevant impacts for this project (sqlx 0.8 → 0.9, SQLite + migrate + macros):
    - Breaking — likely requires code changes:
      - SQLite SqliteValue/SqliteValueRef now !Sync/!Send — if any code holds these across .await points, compile error. Check db.rs.
      - Stricter SQLite type validation per value — query_as! calls in db.rs (lines 115, 131, 144) may fail if column types don't match Rust types exactly. Macros catch this at compile time, but expect new errors.
      - Migrate trait breaking changes — sqlx::migrate!().run(&pool) at db.rs:32 may need updates depending on the exact API change.
    - Breaking — low risk here:
      - SqlSafeStr / AssertSqlSafe — all queries in db.rs use string literals inside query!/query_scalar!/query_as! macros, not format!(). Likely unaffected.
      - runtime-tokio feature — combo features removed (e.g. runtime-tokio-native-tls), not bare runtime-tokio. Safe.
    - Non-breaking:
      - sqlx-toml — opt-in only, no action needed.
      - SQLite extension loading now unsafe — no extension loading in codebase.
      - TransactionManager re-export removed — not used.
    - Action required before upgrading:
      - Rust 1.94.0+ needed (0.9 MSRV). Project has no rust-version pin — check CI toolchain.
      - Compile with --features web after bump and expect macro errors to surface type mismatches.
