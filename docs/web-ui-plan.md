# Saurron Web UI — Implementation Plan

## Scope

Dashboard page only (v1). Design reference: `design/dashboard/project/Saurron Dashboard v4.html`.
Layout: LayoutTimeline (timeline / activity-feed style).
Accent: Saurron default only (no picker).
Search and filter: client-side, over the loaded page window.

---

## Milestones

Each milestone is one commit (or a small cluster of tightly coupled commits) on the `web-ui` branch.

---

### M1 — Rust: Cargo feature, config, dependencies ✓ DONE

**No functional change. Compiles clean in both modes.**

- `Cargo.toml`
  - Add `web` feature flag
  - Add `sqlx` 0.8.6 (with `sqlite`, `runtime-tokio`, `macros`, `migrate` features) gated by `web`
  - Add `include_dir` 0.7.4 gated by `web`
- `web/dist/.gitkeep`: empty placeholder so `include_dir!("web/dist")` compiles before `pnpm build` has run
- `cli.rs`: add `--http-api-web-ui` (bool) and `--db-path` (PathBuf) args — both `#[cfg(feature = "web")]`
- `config.rs`
  - Add `HttpApiConfig.web_ui: bool` — TOML key `web_ui`, env `SAURRON_HTTP_API_WEB_UI` — `#[cfg(feature = "web")]`
  - Add `DbConfig { path: PathBuf }` — TOML section `[db]`, key `path`, env `SAURRON_DB_PATH`, default `/etc/saurron/saurron.db` — `#[cfg(feature = "web")]`
  - Wire both into layered merge
  - `log_settings()` emits `config: web` log line (feature-gated)

**Acceptance:** `cargo build` and `cargo build --features web` both succeed; no existing tests broken. ✓ 388 tests pass.

---

### M2 — Rust: SessionReport refactor ✓ DONE

**Breaking change within the crate; external `POST /v1/update` response shape changes.**

- `update.rs`
  - Replace flat `updated/skipped/failed/rolled_back/up_to_date: Vec<String>` + `up_to_date: usize` fields on `SessionReport` with `containers: Vec<ContainerReport>`
  - Add `ContainerReport { name, outcome: ContainerOutcome, old_image: Option<String>, new_image: Option<String> }`
    - `old_image`: `Some(container.image)` for all outcomes (Updated, RolledBack, Skipped, Failed); `None` for UpToDate
    - `new_image`: `Some(new_image)` for Updated and RolledBack only; `None` for all other outcomes
  - Add `ContainerOutcome` enum: `Updated | RolledBack | Failed | Skipped | UpToDate`
  - Add `started_at: DateTime<Utc>` and `completed_at: DateTime<Utc>` fields to `SessionReport`; `run_cycle` captures start time at entry and sets `completed_at` before returning
  - Update every `record()` call site inside `run_cycle` to pass `old_image` as `Some(container.image)`
- `notifications.rs`
  - Derive name-lists from `containers` in `render_template`; inject both derived name-lists (`updated`, `skipped`, `failed`, `rolled_back`, `up_to_date`) AND `containers` array into MiniJinja context so existing custom templates continue to work
  - Update `should_notify` and all test helpers that construct `SessionReport` with old flat fields to use `ContainerReport` entries instead
- `metrics.rs`: update `record_cycle` to iterate `report.containers` and filter by outcome rather than using removed flat fields; update affected tests
- `http.rs`: update `SessionReport` serialisation; `POST /v1/update` response now includes a `containers` array

**Acceptance:** `cargo test` passes; `POST /v1/update` response includes `containers`.

---

### M3 — Rust: SQLite schema and cycle persistence ✓ DONE

- `migrations/001_initial.sql`: `CREATE TABLE cycles` and `CREATE TABLE cycle_containers` per the schema in `docs/web-ui-design.md`
- `src/db.rs` (new, `#[cfg(feature = "web")]`)
  - `init_pool(path: &Path) -> Result<SqlitePool>` — fails fast: panics/returns `Err` if pool open or migration fails; caller aborts startup
  - `record_cycle(pool, report: &SessionReport, trigger: &str) -> Result<i64>` — reads `started_at`/`completed_at` from `SessionReport`; propagates DB errors to caller
  - `list_cycles(pool, page, per_page) -> Result<(Vec<CycleRow>, i64)>`
  - `get_cycle(pool, id) -> Result<Option<(CycleRow, Vec<CycleContainerRow>)>>`
- `src/lib.rs`: re-export `db` module behind `web` feature
- `main.rs`:
  - Add `pool: Option<SqlitePool>` to `AppStateInner` (compiled only under `#[cfg(feature = "web")]`)
  - Open pool at startup when `config.http_api.web_ui` is true; abort process on failure
  - Pool is `None` when `web_ui` is false (no DB opened)
- `http.rs`: add `trigger: &str` parameter to `run_cycle_with_state`; pass trigger through so each call site supplies the correct value
- Call `db::record_cycle` inside `run_cycle_with_state` when `state.pool` is `Some`, alongside existing `metrics::record_cycle` and `notifications::dispatch` calls; propagate/log any `Err` returned
- Trigger values per call site:
  - scheduler loop in `main.rs` → `"scheduled"`
  - `post_update` HTTP handler → `"http_api"`
  - `--run-once` path in `main.rs` → `"manual"`
- `.sqlx/` offline query cache:
  - Install `sqlx-cli` if not present: `cargo install sqlx-cli --no-default-features --features sqlite`
  - Create a temp DB and run migrations: `DATABASE_URL=sqlite:///tmp/saurron_prepare.db cargo sqlx migrate run --source migrations`
  - Generate cache: `DATABASE_URL=sqlite:///tmp/saurron_prepare.db cargo sqlx prepare --features web`
  - Commit the generated `.sqlx/` directory; CI builds without `DATABASE_URL` using this cache

**Acceptance:** running with `--http-api-web-ui --db-path /tmp/t.db` writes rows after a cycle finishes; `cargo build --features web` succeeds in CI without `DATABASE_URL`.

---

### M4 — Rust: Backend API endpoints ✓ DONE

All new routes compiled only with `--features web`. No auth enforced in v1.

**New endpoints:**

| Method | Path | Response |
|--------|------|----------|
| `GET` | `/v1/history` | `?page=N&per_page=20` → `{ cycles: [...], total: N }` |
| `GET` | `/v1/history/:id` | `{ cycle: {...}, containers: [...] }` |
| `GET` | `/v1/containers` | `[{ name, state }]` where `state` ∈ `"running" \| "monitor_only" \| "pinned"` |
| `GET` | `/ui/*path` | Serve embedded static bundle from `include_dir!("web/dist")` |

**Extended:**
- `GET /v1/health`: add `updating: bool`, `version: string`, `hostname: string` fields

**Notes on `/v1/containers`:**
- Calls both `docker.list_containers()` and `docker.select_containers()` at request time (same two-step pattern as `post_update`)
- Maps each container's labels to one of three states: `monitor_only` if `saurron.monitor-only=true`; `pinned` if `saurron.image-tag` contains a digest or `saurron.no-pull=true`; `running` otherwise
- State precedence: `monitor_only` > `pinned` > `running`

**Notes on `/v1/history` and `/v1/history/:id`:**
- When pool is `None` (web UI disabled), return `503 Service Unavailable`

**Notes on `/v1/health`:**
- Extended fields (`updating`, `version`, `hostname`) are always present — not gated behind `--features web`
- `updating`: `update_lock.try_lock().is_err()`
- `version`: `env!("SAURRON_VERSION")`
- `hostname`: `$HOSTNAME` env var, fallback to `/etc/hostname`, fallback to empty string

**Notes on `GET /ui/*path`:**
- Missing files return `404 Not Found` (no SPA fallback to `index.html`)
- Empty path (`/ui/`) serves `index.html`

**Files touched:** `src/http.rs` (new handlers + response types + route registration)

**Acceptance:** all endpoints return valid JSON; `GET /ui/` returns `index.html` from the embedded bundle.

---

### M5 — Frontend: Scaffold ✓ DONE

`web/` directory at repo root. Independent of Rust build.

**Stack:** Svelte 5, SMUI v9 (no MDCWeb — dropped in v9; SMUI used for behaviour/a11y only), no Sass. All styling via custom CSS tokens.

**Theme:** `data-theme` attribute on `<html>` set by inline script before first paint (avoids flash); default = system preference (`prefers-color-scheme`); persisted to `localStorage`. `App.svelte` wrapper `<div class="saurron-app">` mirrors `data-theme` reactively from a theme store.

**Routing:** `svelte-spa-router` (hash-based). Dev server base path `/ui/`; developer hits `http://localhost:PORT/ui/` in browser.

**Files:**

- `web/package.json`: `svelte`, `vite`, `@sveltejs/vite-plugin-svelte`, `svelte-spa-router`, `@smui/button`, `@smui/card`, `@smui/data-table`, `@smui/drawer`, `@smui/linear-progress`, `@smui/snackbar`, `@smui/textfield`, `material-symbols` (npm package — offline/Docker safe)
- `web/vite.config.js`: Svelte plugin, `base: '/ui/'`, dev proxy (`/v1` → `http://localhost:8080`)
- `web/src/main.js`: Svelte mount entry point
- `web/index.html`: base HTML; inline script reads `localStorage` → `prefers-color-scheme` fallback → sets `document.documentElement.dataset.theme`
- `web/src/tokens.css`: port of `design/dashboard/project/saurron-tokens.css` — Saurron accent only (ember/teal/indigo removed); `.saurron-app` scoping unchanged; light + dark themes; all primitives (shape, elevation, type scale, chips, buttons, cards, running indicators)
- `web/src/App.svelte`: `<div class="saurron-app" data-theme={$theme}>` wrapper; theme store initialised from `document.documentElement.dataset.theme`, writes back to `localStorage`; `svelte-spa-router` `<Router>` with route map for `/`, `/update`, `/template`, `/notifications`
- `web/src/routes/Dashboard.svelte`: empty stub (placeholder content)
- `web/src/routes/Update.svelte`: empty stub
- `web/src/routes/Template.svelte`: empty stub
- `web/src/routes/Notifications.svelte`: empty stub
- `web/src/lib/api.js`: fetch wrapper: `getHistory(page, perPage)`, `getHistoryById(id)`, `getHealth()`, `getContainers()`

**Acceptance:** `pnpm dev` starts Vite; navigating to `http://localhost:PORT/ui/` renders a shell with correct CSS tokens applied in both light and dark themes.

---

### M6 — Frontend: App chrome ✓ DONE

NavDrawer, TopAppBar, shared atoms.

**Decisions recorded:**

- `App.svelte` owns the layout shell (Option A): NavDrawer + TopAppBar wrap the `<Router>` once; route stubs remain simple. All v1 routes use identical chrome.
- `CycleStatusCard` idle state shows placeholder values (`—` / `—` / `—`) for next-cycle countdown, schedule interval, and watched count; `/v1/health` does not expose this data and no other endpoint does either. Wire to real data when a future endpoint provides it.
- `CycleStatusCard` running state shows indeterminate progress bar + "Cycle running" label only; no scanned/total/current-container-name because `/v1/health` only exposes `updating: bool`.
- `BottomNavBar` shows the four nav destinations only (Dashboard, Manual update, Template, Notifications). No "Run cycle" button in the bottom bar.
- `TopAppBar` includes a non-functional search field (input renders, ⌘K shortcut does nothing); wired up in M8.
- `material-symbols/rounded.css` already imported in `main.js` (done in M5); no font-loading work needed in M6.

**Deliverables:**

- `web/src/App.svelte` (update): wrap `<Router>` in a layout shell — `NavDrawer` on the left, right column with `TopAppBar` above and `<Router>` below; `BottomNavBar` at bottom of viewport on small screens
- `web/src/lib/NavDrawer.svelte`
  - Standard variant (264 px): logo, version + hostname from health store, RunningChip, nav items (active state), Watched section (from `getContainers`), CycleStatusCard footer
  - Rail variant (64 px): logo with running-dot badge, icon-only nav buttons, CycleStatusBadge footer
  - Responsive: standard at ≥900 px viewport width; rail at ≥600 px; hidden below 600 px (bottom bar takes over)
- `web/src/lib/TopAppBar.svelte`
  - Title + optional subtitle
  - Non-functional search field (input present, ⌘K does nothing; wired in M8)
  - Theme toggle button (light_mode / dark_mode icon)
  - "Run cycle" filled button (routes to `/update` page); morphs to disabled "Running…" tonal button while `health.updating` is true
  - Indeterminate progress bar underline (`running-bar thin`) while cycle running
- `web/src/lib/BottomNavBar.svelte`: four icon+label nav destinations (Dashboard, Manual update, Template, Notifications); shown below 600 px viewport width
- `web/src/lib/RunningChip.svelte`: idle / live pill
- `web/src/lib/CycleStatusCard.svelte`:
  - Idle state: "Next cycle" card with placeholder values (`—`) for countdown, schedule interval, watched count
  - Running state: indeterminate progress bar + "Cycle running" label; no scanned/total/current (health only provides `updating: bool`)
- `web/src/lib/atoms/OutcomeChip.svelte`: `outcome` prop → colour class + icon
- `web/src/lib/atoms/TriggerChip.svelte`: `trigger` prop → icon + small-caps label
- `web/src/stores/health.js`: Svelte writable store; polls `GET /v1/health` every 5 s; exposes `{ updating, version, hostname }`

**Acceptance:** drawer renders with nav items; cycle running indicators activate when `health.updating` is true (simulate by hard-coding `true` in the store); theme toggle persists in `localStorage` across refresh.

---

### M7 — Frontend: Dashboard stat cards ✓ DONE

Summary strip at the top of Dashboard.

**Decisions recorded:**

- "This week" means rolling 7 days (not calendar week), consistent with M8's default "Last 7 days" filter.
- `StatCard` badge/chart use Svelte 5 snippet syntax (`{#snippet}` / `{@render}`), not legacy `<slot>`.
- Sparkline y-axis plots `updated` count per day; bad-day dots mark any day with `failed > 0` or `rolled_back > 0`.
- Responsive layout uses CSS media queries (no JS `compact` prop). Sparkline SVG uses `width="100%"` + `viewBox` so CSS controls its width. A viewport store is deferred to M8 if JS-gated compact behaviour is needed there.
- `duration_sec` is not in the API response; computed on the frontend as `(new Date(completed_at) - new Date(started_at)) / 1000`.
- "Updates this week" subtitle shows `across N cycles` where N = cycle count in the rolling 7-day window.
- Sparkline always emits exactly 7 data points; days with no cycles are zero-padded.
- When history is empty, stat cards show `—` for all derived values.

**Deliverables:**

- `web/src/lib/StatCard.svelte`: label (overline), large display value, subtitle, optional badge snippet, optional chart snippet; `tone` prop drives error-container background when failures exist
- `web/src/lib/Sparkline.svelte`: SVG polyline + filled area; `width="100%"` + `viewBox` for CSS-controlled sizing; data is `{ updated, failed, rolled_back }[]` per day (7 points, zero-padded); bad-day dots rendered in `--error` colour
- `Dashboard.svelte` additions:
  - On mount: fetch page 1 of history (`getHistory(1, 100)` to get enough data for weekly stats)
  - Derive: last cycle timestamp + outcome (failed > 0 → `failed`; rolled_back > 0 → `rolled_back`; updated > 0 → `updated`; else `up_to_date`), updates-this-week, failures-this-week, cycle-count-this-week, daily aggregates (7 points) for sparkline
  - Render three `StatCard` instances in a responsive 3-column (desktop) / 2-column (≤899 px) grid; Last Cycle card spans full width at ≤899 px

**Acceptance:** cards show real API values; sparkline renders without errors on empty dataset; failure card has error-container tint when count > 0.

---

### M8 — Frontend: Timeline feed, filters, search ✓ DONE

Main content area of Dashboard.

**Decisions recorded:**

- Initial history fetch is `getHistory(1, 20)` (not 100). Stat cards are computed from all loaded pages; weekly stats may undercount if 7-day history spans more pages than loaded — acceptable for v1.
- "Load older" performs real API pagination (page 2, 3, …, per_page=20) and appends to local list. `total` from API response drives whether the button is shown.
- Search query is shared via a new `src/stores/search.js` writable store. `TopAppBar` writes on input; `Dashboard` reads and filters. State persists across route changes — acceptable for v1.
- Compact/desktop branching (segmented toggle visibility, preview chip visibility) is handled with CSS `@media` rules only. No JS viewport store.
- `ContainerOutcomeRow` new-image column: shows `new_image` tag + digest when present; falls back to `reason` or `—` when `new_image` is null (covers `skipped` and some `failed` cases).

**Deliverables:**

- `web/src/lib/TimelineEvent.svelte`
  - Collapsed row: time (HH:MM monospace), duration, TriggerChip, outcome summary text, preview chips for changed containers (up to 4), expand chevron
  - Expanded panel: cycle metadata (id, abs timestamp, scanned, duration, completed time), `ContainerOutcomeRow` list (non-up-to-date first, then "+ N up to date" footer)
  - Quiet cycles (all up-to-date) rendered at reduced opacity with condensed row
- `web/src/lib/ContainerOutcomeRow.svelte`: 5-column grid — name (with colour dot) · old-image tag + short digest · arrow → · new-image tag / reason · OutcomeChip
- `web/src/lib/FilterDropdown.svelte`: pill button opens a floating card with radio-style options; check icon on active item; closes on outside click
- `Dashboard.svelte` additions:
  - Day-grouped sections: day header (calendar icon, label, cycle count, updated count, incidents count) + vertical timeline rule
  - "Load older" button: fetches next page from API and appends to local list
  - Time-window filter (Last 24 h / 7 d / 30 d / 90 d / All time): applied client-side over loaded data; operates on `started_at`
  - Trigger filter (All / Scheduled / Manual / HTTP API): applied client-side
  - Segmented All / Updates / Failures toggle (desktop only): quick filter that hides quiet cycles (Updates) or shows only failed/rolled-back cycles (Failures)
  - Search field in TopAppBar (⌘K focuses): filters visible events by container name substring match (case-insensitive, across all loaded pages)
  - Empty state component when no cycles exist yet

**Acceptance:** expanding a row shows per-container detail; filters narrow visible set without extra network requests; search narrows by container name; "Load older" appends next page; empty state renders correctly.

---

### M9 — Build pipeline integration ✓ DONE

Wire frontend build into the release process.

**Dockerfiles — all 6** (`docker/bookworm/full`, `docker/bookworm/slim`, `docker/bullseye/full`, `docker/bullseye/slim`, `docker/trixie/full`, `docker/trixie/slim`):

- Insert a new `node` build stage before the `rust` builder stage:
  - Base image: `node:22-<release>` (or `node:22-<release>-slim` for slim variants)
  - `WORKDIR /build/web`
  - `COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml ./`
  - `RUN npm install -g pnpm && pnpm install --frozen-lockfile`
  - `COPY web/ ./`
  - `RUN pnpm build`
- In the `rust` builder stage, before `cargo build`:
  - `COPY --from=node /build/web/dist ./web/dist`
- Change `cargo build --profile release --locked` → `cargo build --profile release --locked --features web`

**CI — new workflow `.github/workflows/web-build.yml`:**

- Triggers: `push` to `main` and `pull_request` to `main`, path-filtered to `web/**` (same paths as `web-lint.yml` plus `web/vite.config.js`, `web/svelte.config.js`, `web/index.html`)
- Single `build` job: `pnpm install --frozen-lockfile && pnpm build`; Node 22, `pnpm/action-setup`, `working-directory: web`

**CI — `rust.yml` updates:**

- `lint` job: `cargo clippy --all-targets -- -D warnings` → `cargo clippy --all-targets --features web -- -D warnings`
- `build` matrix job: `cargo build --profile release` → `cargo build --profile release --features web`; same for both `cargo test` invocations

**CI — `docker.yml` updates:**

- `binary-build` job: `cargo build --profile release --locked` → `cargo build --profile release --locked --features web`; add frontend build steps before `cargo build` (install Node 22 via `actions/setup-node`, install pnpm, run `pnpm install --frozen-lockfile && pnpm build` in `web/`)

**Other:**

- `web/.gitignore`: exclude `node_modules/`, `dist/`
- Root `CLAUDE.md` `## Commands` section: add `pnpm install` and `pnpm build` entries (note: run from `web/` directory)

**Acceptance:** `docker build` produces an image where `GET /ui/` returns the Svelte bundle; CI passes on both feature modes; clippy checks feature-gated code.

---

## Dependency graph

```
M1 ──► M2 ──► M3 ──► M4
                           \
M5 ──► M6 ──► M7 ──► M8 ──► M9
```

M1–M4 are pure Rust. M5–M8 are pure frontend. Both tracks can proceed in parallel after M1 is committed, since the frontend uses the Vite proxy for local dev and the mock data for visual development.

---

## Out of scope (v1)

- Manual Update, Template Preview, Test Notifications pages
- Accent color picker (Ember, Teal, Indigo accents)
- Authentication on new endpoints
- Live cycle progress via SSE
- Server-side history search / filtering
- Notification target configuration UI
- Cleanup incomplete cycles in `cycles` table
- Show count/list of _disabled_ containers excluded from processing
