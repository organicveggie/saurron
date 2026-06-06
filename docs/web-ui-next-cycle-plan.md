# Next Cycle Card — Implementation Plan

**Goal:** Replace the three placeholder values in the idle `CycleStatusCard` with real data:
- Countdown line: `in 42m` → time until next scheduled run
- Sub-label: `every 1h · 30 watched` → schedule description + watched container count

**Design reference:** `design/dashboard/project/saurron-chrome.jsx` line 217.

---

## Data requirements

| Value | Source | How |
|---|---|---|
| Countdown (`in 42m`) | `next_run_at` timestamp | Backend computes after each cycle, stored in shared state; returned in `/v1/health` |
| Schedule label (`every 1h` / `0 3 * * *` / `(run once only)`) | Config at startup | Stored in `AppStateInner` as `ScheduleInfo`; returned in `/v1/health` |
| Watched count (`30 watched`) | Container list | Already fetched in `NavDrawer` via `/v1/containers`; passed as prop |

---

## Design decisions

| Question | Decision |
|---|---|
| Countdown before first cycle completes (interval mode — first run is immediate) | Show `—` until scheduler writes first `next_run_at` |
| Countdown for cron mode before first cycle | Set `next_run_at` immediately at startup (before first sleep); UI shows real countdown from launch |
| Cron mode sub-label | Raw cron expression (e.g. `0 3 * * *`) — use `s.source().to_string()` from `cron::Schedule` |
| Countdown tick rate | Reuse 5 s health poll — no extra `setInterval` |
| `run_once` mode | `run_scheduler` not called; `next_run_at` stays `None`; sub-label shows `(run once only) · N watched` |

---

## Backend changes

### 1. `src/http.rs` — new `ScheduleInfo` type + two new fields on `AppStateInner`

Add above `AppStateInner`:

```rust
pub struct ScheduleInfo {
    pub mode: &'static str,          // "interval" | "cron" | "run_once"
    pub interval_secs: Option<u64>,
    pub cron_expr: Option<String>,
}
```

Add to `AppStateInner`:

```rust
pub schedule_info: ScheduleInfo,
pub next_run_at: std::sync::Mutex<Option<chrono::DateTime<chrono::Utc>>>,
```

`schedule_info` is written once at startup and never changes — no async lock needed.
`next_run_at` uses `std::sync::Mutex` (not tokio) because the callback that writes it is sync.

### 2. `src/scheduler.rs` — add `on_next_run` callback to `run_scheduler`

Change signature:

```rust
pub async fn run_scheduler<F, Fut, N>(mode: ScheduleMode, run_cycle: F, on_next_run: N)
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = ()>,
    N: Fn(Option<chrono::DateTime<chrono::Utc>>),
```

Callback call sites:

| Mode | When to call | Argument |
|---|---|---|
| `RunOnce` | N/A — `run_scheduler` not called for `RunOnce` in `main.rs` (direct dispatch); `next_run_at` stays `None` by default, which is correct | — |
| `Interval` | After cycle, before sleep | `Some(Utc::now() + duration)` |
| `Cron` | Top of loop, before each sleep (fires before first sleep too, so `next_run_at` is set at startup) | `Some(next_fire_utc)` |

For `Cron`, move the callback to the top of the loop and remove the redundant post-cycle `schedule.upcoming(Utc).next()` log call — the next loop iteration recomputes it anyway.

All existing unit tests that call `run_scheduler` add a no-op third arg: `\|_\| {}`.

### 3. `src/main.rs` — wire `ScheduleInfo`, `next_run_at`, and the callback

Build `ScheduleInfo` from `schedule_mode` before constructing `AppStateInner`:

```rust
let schedule_info = match &schedule_mode {
    ScheduleMode::RunOnce => ScheduleInfo { mode: "run_once", interval_secs: None, cron_expr: None },
    ScheduleMode::Interval(d) => ScheduleInfo { mode: "interval", interval_secs: Some(d.as_secs()), cron_expr: None },
    ScheduleMode::Cron(s) => ScheduleInfo { mode: "cron", interval_secs: None, cron_expr: Some(s.source().to_string()) },
};
```

> Note: `cron::Schedule` exposes `.source()` (returns `&str`, stable public API). Prefer this over `config.schedule.clone().unwrap_or_default()` — the latter produces `""` in a branch where `schedule` is always `Some`, which is misleading.

Add to `AppStateInner` construction:

```rust
schedule_info,
next_run_at: std::sync::Mutex::new(None),
```

Pass callback to `run_scheduler`:

```rust
let next_run_state = Arc::clone(&state);
scheduler::run_scheduler(schedule_mode, cycle_fn, move |t| {
    if let Ok(mut g) = next_run_state.next_run_at.lock() {
        *g = t;
    }
}).await;
```

### 4. `src/http.rs` — extend `/v1/health` response

```rust
let next_run = state.next_run_at.lock().ok()
    .and_then(|g| *g)
    .map(|t| t.to_rfc3339());

Json(serde_json::json!({
    "status": "ok",
    "updating": updating,
    "version": VERSION,
    "hostname": hostname,
    "schedule_mode": state.schedule_info.mode,
    "schedule_interval_secs": state.schedule_info.interval_secs,
    "schedule_cron": state.schedule_info.cron_expr,
    "next_run_at": next_run,
}))
```

---

## Frontend changes

### 5. `web/src/stores/health.js` — add new fields to store shape and polling

Extend initial value and `health.set(...)` call:

```js
export const health = writable({
  updating: false,
  version: '',
  hostname: '',
  schedule_mode: '',
  schedule_interval_secs: null,
  schedule_cron: null,
  next_run_at: null,
});
```

Map new fields from `d` in the poll function:

```js
schedule_mode: d.schedule_mode ?? '',
schedule_interval_secs: d.schedule_interval_secs ?? null,
schedule_cron: d.schedule_cron ?? null,
next_run_at: d.next_run_at ?? null,
```

### 6. `web/src/lib/CycleStatusCard.svelte` — consume real data

Add `watchedCount` prop and read from health store:

```svelte
<script>
  import { health } from '../stores/health.js';

  let { running = false, watchedCount = 0 } = $props();

  function fmtCountdown(isoStr) {
    if (!isoStr) return '—';
    const secs = Math.round((new Date(isoStr) - Date.now()) / 1000);
    if (secs <= 0) return 'now';
    if (secs < 60) return `in ${secs}s`;
    const mins = Math.round(secs / 60);
    if (mins < 60) return `in ${mins}m`;
    const hours = Math.floor(mins / 60);
    const rem = mins % 60;
    return rem > 0 ? `in ${hours}h ${rem}m` : `in ${hours}h`;
  }

  function scheduleLabel(h) {
    if (h.schedule_mode === 'run_once') return '(run once only)';
    if (h.schedule_mode === 'interval') {
      const s = h.schedule_interval_secs;
      if (!s) return '—';
      if (s % 3600 === 0) return `every ${s / 3600}h`;
      if (s % 60 === 0) return `every ${s / 60}m`;
      return `every ${s}s`;
    }
    if (h.schedule_mode === 'cron') return h.schedule_cron ?? '—';
    return '—';
  }

  let countdown = $derived(fmtCountdown($health.next_run_at));
  let sched = $derived(scheduleLabel($health));
  let sub = $derived(`${sched} · ${watchedCount} watched`);
</script>
```

Replace placeholder lines in idle branch:

```svelte
<div class="type-mono type-num countdown">{countdown}</div>
<div class="type-body-sm" style="color: var(--on-surface-muted)">{sub}</div>
```

### 7. `web/src/lib/NavDrawer.svelte` — pass `watchedCount` to `CycleStatusCard`

Change:

```svelte
<CycleStatusCard running={$health.updating} />
```

To:

```svelte
<CycleStatusCard running={$health.updating} watchedCount={containers.length} />
```

No other change to NavDrawer — `containers` array is already populated in `onMount`.

---

## Edge cases

| Scenario | Behaviour |
|---|---|
| Interval — before first cycle completes | `next_run_at` is `null`; countdown shows `—` |
| `run_once` mode | `next_run_at` stays `null`; sub-label shows `(run once only) · N watched` |
| `next_run_at` already passed (overdue cycle) | `fmtCountdown` returns `now` |
| Backend running without `--http-api-*` | Not applicable — health endpoint is always active |
| `schedule_mode` field absent (old server) | Frontend falls back to `''`; `scheduleLabel` returns `—` |

---

## Files changed

| File | Change |
|---|---|
| `src/http.rs` | Add `ScheduleInfo` struct + 2 fields to `AppStateInner`; extend health JSON |
| `src/scheduler.rs` | Add `on_next_run` param to `run_scheduler`; write callback in all three `ScheduleMode` arms |
| `src/main.rs` | Build `ScheduleInfo`; add `next_run_at` mutex; pass callback to `run_scheduler` |
| `web/src/stores/health.js` | Extend store shape and poll mapping |
| `web/src/lib/CycleStatusCard.svelte` | Add `watchedCount` prop; read health store; compute countdown + sub-label |
| `web/src/lib/NavDrawer.svelte` | Pass `watchedCount={containers.length}` to `CycleStatusCard` |

No new dependencies. No schema migrations. No new API endpoints.

---

## Test impact

- `scheduler.rs` unit tests: add `|_| {}` third arg to each `run_scheduler` call.
- No new tests required: the countdown/label logic is pure JS, easy to verify manually. The `on_next_run` callback path is exercised by the existing async scheduler tests once the signature is updated.
- `http.rs` unit tests: `AppStateInner` construction adds two fields — update all `AppStateInner { ... }` literals in tests.
