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

### M1 — Rust: Cargo feature, config, dependencies

**No functional change. Compiles clean in both modes.**

- `Cargo.toml`
  - Add `web` feature flag
  - Add `sqlx` (with `sqlite`, `runtime-tokio`, `macros`, `migrate` features) gated by `web`
  - Add `include_dir` gated by `web`
- `web/dist/.gitkeep`: empty placeholder so `include_dir!("web/dist")` compiles before `pnpm build` has run
- `cli.rs`: add `--http-api-web-ui` (bool) and `--db-path` (PathBuf) args
- `config.rs`
  - Add `HttpApiConfig.web_ui: bool` — TOML key `web_ui`, env `SAURRON_HTTP_API_WEB_UI`
  - Add `DbConfig { path: PathBuf }` — TOML section `[db]`, key `path`, env `SAURRON_DB_PATH`, default `/etc/saurron/saurron.db`
  - Wire both into layered merge

**Acceptance:** `cargo build` and `cargo build --features web` both succeed; no existing tests broken.

---

### M2 — Rust: SessionReport refactor

**Breaking change within the crate; external `POST /v1/update` response shape changes.**

- `update.rs`
  - Replace flat `updated/skipped/failed/rolled_back/up_to_date: Vec<String>` + `up_to_date: usize` fields on `SessionReport` with `containers: Vec<ContainerReport>`
  - Add `ContainerReport { name, outcome: ContainerOutcome, old_image: Option<String>, new_image: Option<String> }`
  - Add `ContainerOutcome` enum: `Updated | RolledBack | Failed | Skipped | UpToDate`
  - Add `started_at: DateTime<Utc>` and `completed_at: DateTime<Utc>` fields to `SessionReport`; `run_cycle` captures start time at entry and sets `completed_at` before returning
  - Update every `record()` call site inside `run_cycle` to pass `old_image`
- `notifications.rs`: derive name-lists from `containers` in `render_template`; update MiniJinja context so existing custom templates continue to work
- `http.rs`: update `SessionReport` serialisation; `POST /v1/update` response now includes a `containers` array
- All unit tests touching `SessionReport` field names

**Acceptance:** `cargo test` passes; `POST /v1/update` response includes `containers`.

---

### M3 — Rust: SQLite schema and cycle persistence

- `migrations/001_initial.sql`: `CREATE TABLE cycles` and `CREATE TABLE cycle_containers` per the schema in `docs/web-ui-design.md`
- `src/db.rs` (new, `#[cfg(feature = "web")]`)
  - `init_pool(path: &Path) -> SqlitePool`
  - `record_cycle(pool, report: &SessionReport, trigger: &str) -> Result<i64>` — reads `started_at`/`completed_at` from `SessionReport`
  - `list_cycles(pool, page, per_page) -> Result<(Vec<CycleRow>, i64)>`
  - `get_cycle(pool, id) -> Result<Option<(CycleRow, Vec<CycleContainerRow>)>>`
- `src/lib.rs`: re-export `db` module behind `web` feature
- `main.rs`: open pool on startup when `config.http_api.web_ui` is true; add to `AppStateInner`
- Call `db::record_cycle` from each call site that already knows the trigger — scheduler (`"scheduled"`), `post_update` HTTP handler (`"http_api"`), CLI `--run-once` path (`"manual"`) — alongside existing `metrics::record_cycle` and `notifications::dispatch` calls
- `.sqlx/` offline query cache: run `cargo sqlx prepare --features web` and commit

**Acceptance:** running with `--http-api-web-ui --db-path /tmp/t.db` writes rows after a cycle finishes; `cargo build --features web` succeeds in CI without `DATABASE_URL`.

---

### M4 — Rust: Backend API endpoints

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
- Calls `docker.list_containers()` at request time (returns only selected containers)
- Maps each container's labels to one of three states: `monitor_only` if `saurron.monitor-only=true`; `pinned` if `saurron.image-tag` contains a digest or `saurron.no-pull=true`; `running` otherwise

**Files touched:** `src/http.rs` (new handlers + response types + route registration)

**Acceptance:** all endpoints return valid JSON; `GET /ui/` returns `index.html` from the embedded bundle.

---

### M5 — Frontend: Scaffold

`web/` directory at repo root. Independent of Rust build.

- `web/package.json`: `svelte`, `vite`, `@sveltejs/vite-plugin-svelte`, `svelte-spa-router`, `@smui/button`, `@smui/card`, `@smui/data-table`, `@smui/drawer`, `@smui/linear-progress`, `@smui/snackbar`, `@smui/textfield`, `material-symbols`
- `web/vite.config.js`: Svelte plugin + dev proxy (`/v1` → `http://localhost:8080`)
- `web/index.html`: base HTML; inline script applies `data-theme` from `localStorage` before first paint to avoid theme flash
- `web/src/tokens.css`: port of `design/dashboard/project/saurron-tokens.css` — Saurron accent only; light + dark themes; all primitives (shape, elevation, type scale, chips, buttons, cards, running indicators)
- `web/src/App.svelte`: router shell, theme store initialisation, `svelte-spa-router` `<Router>` with route map stubs for `/`, `/update`, `/template`, `/notifications`
- `web/src/routes/Dashboard.svelte`: empty stub (renders chrome + placeholder content)
- `web/src/lib/api.js`: fetch wrapper: `getHistory(page, perPage)`, `getHistoryById(id)`, `getHealth()`, `getContainers()`

**Acceptance:** `pnpm dev` starts Vite; navigating to `/` renders a shell with correct CSS tokens applied in both light and dark themes.

---

### M6 — Frontend: App chrome

NavDrawer, TopAppBar, shared atoms.

**Deliverables:**

- `web/src/lib/NavDrawer.svelte`
  - Standard variant (264 px): logo, version + hostname from health store, RunningChip, nav items (active state), Watched section (from `getContainers`), CycleStatusCard footer
  - Rail variant (64 px): logo with running-dot badge, icon-only nav buttons, CycleStatusBadge footer
  - Responsive: standard at ≥900 px viewport width; rail at ≥600 px; hidden below 600 px (bottom bar takes over)
- `web/src/lib/TopAppBar.svelte`
  - Title + optional subtitle
  - Theme toggle button (light_mode / dark_mode icon)
  - "Run cycle" filled button (routes to `/update` page); morphs to disabled "Running…" tonal button while `health.updating` is true
  - Indeterminate progress bar underline (`running-bar thin`) while cycle running
- `web/src/lib/BottomNavBar.svelte`: four icon+label destinations; shown below 600 px
- `web/src/lib/RunningChip.svelte`: idle / live pill
- `web/src/lib/CycleStatusCard.svelte`: idle (Next cycle countdown, schedule interval, watched count) / running (progress bar, current container name, scanned/total count) states
- `web/src/lib/atoms/OutcomeChip.svelte`: `outcome` prop → colour class + icon
- `web/src/lib/atoms/TriggerChip.svelte`: `trigger` prop → icon + small-caps label
- `web/src/stores/health.js`: Svelte writable store; polls `GET /v1/health` every 5 s; exposes `{ updating, version, hostname }`

**Acceptance:** drawer renders with nav items; cycle running indicators activate when `health.updating` is true (simulate by hard-coding `true` in the store); theme toggle persists in `localStorage` across refresh.

---

### M7 — Frontend: Dashboard stat cards

Summary strip at the top of Dashboard.

**Deliverables:**

- `web/src/lib/StatCard.svelte`: label (overline), large display value, subtitle, optional badge slot, optional chart slot; `tone` prop drives error-container background when failures exist
- `web/src/lib/Sparkline.svelte`: SVG polyline + filled area; data is `{ updated, failed, rolled_back }[]` per day (7 points); bad-day dots (any failure) rendered in `--error` colour
- `Dashboard.svelte` additions:
  - On mount: fetch page 1 of history (`getHistory(1, 100)` to get enough data for weekly stats)
  - Derive: last cycle timestamp + outcome, updates-this-week, failures-this-week, daily aggregates for sparkline
  - Render three `StatCard` instances in a responsive 3-column (desktop) / 2-row (compact) grid

**Acceptance:** cards show real API values; sparkline renders without errors on empty dataset; failure card has error-container tint when count > 0.

---

### M8 — Frontend: Timeline feed, filters, search

Main content area of Dashboard.

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

### M9 — Build pipeline integration

Wire frontend build into the release process.

- `Dockerfile` (or `docker-bake.hcl`): add `pnpm install && pnpm build` step in the build stage; move `web/dist/` into the Rust build context before `cargo build --release --features web`
- `.github/workflows/` (or equivalent CI): add a `build-web` job that runs `pnpm install && pnpm build`; existing Rust CI job gains `--features web` when building release artifacts
- `web/.gitignore`: exclude `node_modules/`, `dist/`
- Root `CLAUDE.md` `## Commands` section: add `pnpm install` and `pnpm build` entries

**Acceptance:** `docker build` produces an image where `GET /ui/` returns the Svelte bundle; CI passes on both feature modes.

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
