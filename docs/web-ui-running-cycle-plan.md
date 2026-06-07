# Running-cycle progress plan

## Goal

`CycleStatusCard` running state shows: current container name, scanned/total counter, elapsed
time, and phase label ("scanning" or "updating").

Design reference: `design/dashboard/project/saurron-chrome.jsx` — `CycleStatusCard` (line 208).

## Decisions

- **Phase A + D combined**: Phase A populates `scanned`/`total`; Phase D keeps
  `scanned=total` and updates `current` as each stale container is processed.
- **Elapsed**: computed client-side from `started_at` ISO timestamp returned by the API.
- **Poll rate**: unchanged at 5 s.

---

## Backend — `src/update.rs`

### 1. Add `CycleProgress` struct

Lives in `update.rs` alongside `SessionReport` and other cycle-state types. `http.rs` already
imports from `update`, so no new dependency arc and no circular dependency.

```rust
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CycleProgress {
    pub total: usize,
    pub scanned: usize,
    pub current: String,
    pub phase: String,          // "scanning" | "updating"
    pub started_at: chrono::DateTime<chrono::Utc>,
}
```

---

## Backend — `src/http.rs`

### 3. Add field to `AppStateInner`

```rust
pub cycle_progress: tokio::sync::RwLock<Option<update::CycleProgress>>,
```

Initialised as `RwLock::new(None)` in `main.rs`.

`tokio::sync::RwLock` required — write lock is held across `.await` points inside `run_cycle`.

### 4. Expose in `/v1/health`

Read `cycle_progress` under a read-lock and include as a nullable JSON field:

```json
{
  "updating": true,
  "cycle_progress": {
    "total": 10,
    "scanned": 3,
    "current": "nginx",
    "phase": "scanning",
    "started_at": "2025-01-01T00:00:00Z"
  }
}
```

When idle: `"cycle_progress": null`.

### 5. Add progress handle to `UpdateEngine`

```rust
pub struct UpdateEngine<'a> {
    docker:    &'a docker::DockerClient,
    registry:  &'a registry::RegistryClient,
    config:    &'a config::Config,
    progress:  Option<Arc<tokio::sync::RwLock<Option<CycleProgress>>>>,
}
```

`new()` gains an optional `progress` parameter; a private `update_progress()` helper does
the write under the lock so callers don't repeat boilerplate.

### 6. Wire progress in `run_cycle_with_state` and `post_update` (`http.rs`)

Both call sites (`run_cycle_with_state` for the scheduler and `post_update` for HTTP-triggered cycles) must be wired identically — each creates `UpdateEngine` directly and holds `update_lock` for the full cycle duration.

```rust
let engine = update::UpdateEngine::new(
    &state.docker, &state.registry, &state.config,
    Some(Arc::clone(&state.cycle_progress)),
);
let report = engine.run_cycle(&selected).await;
// clear progress after cycle ends
*state.cycle_progress.write().await = None;
```

### 7. Phase A — update progress per container

Enumerate the loop (`for (i, container) in containers.iter().enumerate()`). Before checking each container:

```rust
self.update_progress(CycleProgress {
    total: containers.len(),
    scanned: i,              // 0-based: containers checked so far
    current: container.name.clone(),
    phase: "scanning".into(),
    started_at,
});
```

No final update after Phase A — Phase D's first write (with `scanned = stale.len()`) covers the transition.

### 8. Phase D — update progress per update

At the start of each stale-container update (before pull/stop/recreate):

```rust
self.update_progress(CycleProgress {
    total:      stale.len(),        // stale count, not full container count
    scanned:    stale.len(),        // Phase A complete
    current:    container.name.clone(),
    phase:      "updating".into(),
    started_at,
});
```

`started_at` is captured once at the top of `run_cycle` and threaded through.

---

## Frontend — `web/src/stores/health.js`

### 9. Extend health store

Add `cycle_progress: null` to the initial writable value. In `poll()`:

```js
health.set({
  ...existing fields...,
  cycle_progress: d.cycle_progress ?? null,
});
```

---

## Frontend — `web/src/lib/NavDrawer.svelte`

### 10. Pass progress to `CycleStatusCard`

```svelte
<CycleStatusCard progress={$health.cycle_progress} watchedCount={containers.length} />
```

Remove the `running={$health.updating}` prop (card derives `running` from `progress != null`).

---

## Frontend — `web/src/lib/CycleStatusCard.svelte`

### 11. Update props and running template

Replace `running: bool` prop with `progress` (object or null). Derive `running` locally:

```js
let { progress = null, watchedCount = 0 } = $props();
let running = $derived(progress !== null);
```

Running card body (matches design mockup):

```svelte
{#if running}
  <div class="card outlined cycle-card running">
    <div class="row">
      <span class="running-dot"></span>
      <span class="type-label" style="color: var(--primary)">Cycle running</span>
      <div style="flex:1"></div>
      <span class="type-mono elapsed">{elapsed}</span>
    </div>
    <div class="running-bar"></div>
    <div class="row justify-between">
      <span class="type-mono current">{progress.current}</span>
      <span class="type-mono count">{progress.scanned}/{progress.total}</span>
    </div>
    <div class="type-body-sm muted">
      {progress.phase} · {pct}% complete
    </div>
  </div>
```

Derived values:

```js
let pct     = $derived(progress?.total > 0
                ? Math.round((progress.scanned / progress.total) * 100) : 0);
let elapsed = $derived(fmtElapsed(progress?.started_at));
```

`fmtElapsed(isoStr)` computes `Date.now() - new Date(isoStr)` and formats as `0:42` or `1:23`.

To drive re-computation between polls, add a 1 s tick store:

```js
import { readable } from 'svelte/store';
const tick = readable(0, set => {
  const id = setInterval(() => set(Date.now()), 1000);
  return () => clearInterval(id);
});
```

Reference `$tick` in the `elapsed` derived expression so Svelte re-evaluates every second:

```js
let elapsed = $derived(fmtElapsed(progress?.started_at, $tick));
```

---

## File change summary

| File | Change |
|---|---|
| `src/update.rs` | Add `CycleProgress` struct; add `progress` field to `UpdateEngine`; update Phase A and Phase D loops |
| `src/http.rs` | Add `cycle_progress: RwLock<Option<update::CycleProgress>>` to `AppStateInner`; wire into `run_cycle_with_state`; expose in `/v1/health` |
| `src/main.rs` | Initialise `cycle_progress: RwLock::new(None)` in `AppStateInner` construction |
| `web/src/stores/health.js` | Add `cycle_progress` to initial state and `poll()` mapping |
| `web/src/lib/NavDrawer.svelte` | Pass `progress={$health.cycle_progress}` instead of `running={$health.updating}` |
| `web/src/lib/CycleStatusCard.svelte` | Replace `running: bool` prop with `progress` object; add counter, current name, elapsed, phase label |

---

## Out of scope

- Poll-rate increase (user decision: keep 5 s).
- Progress for Phase B (inspect) and Phase C (sort) — both are fast; not worth tracking.
- Phase D self-update step (deferred-to-end container) — treated the same as any Phase D container.
