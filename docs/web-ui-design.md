# Saurron Web UI — Design Document

## Overview

SPA built with Svelte + SMUI, served as static bundle by existing Axum HTTP server. Provides update history dashboard, manual update triggers, custom template preview, test notification dispatch. Follows MD3 including light/dark mode.

Web UI optional, gated by new `http_api.web_ui` config flag alongside existing `http_api.update` and `http_api.metrics`.

---

## Architecture

### Rendering model

**Svelte SPA (client-side rendering).** Svelte compiles to small optimised JS/CSS bundle, no runtime framework overhead. Axum serves bundle as static files; browser downloads once, handles all navigation client-side. SPA calls Saurron's HTTP API for all data.

SvelteKit SSR considered and rejected: requires persistent Node.js server process in production, complicates deployment, adds second runtime dependency. Svelte compiled output renders fast enough client-side that blank-loading-state concern is not material for operator dashboard with modest data volumes.

### Integration with the existing server

Axum serves compiled Svelte bundle from `/ui` path when `http_api.web_ui` enabled. SPA communicates with Saurron exclusively through HTTP API. Five new REST endpoints added alongside existing three:

```
Axum router
├── /v1/update                   (existing)
├── /v1/health                   (existing)
├── /v1/metrics                  (existing)
├── /v1/history                  (new — paginated cycle list)
├── /v1/history/:id              (new — single cycle with per-container detail)
├── /v1/template                 (new — return currently configured template)
├── /v1/template/preview         (new — render template with synthetic data)
├── /v1/notifications/test       (new — send test notification to a target)
└── /ui/*                        (new — static SPA bundle)
```

Bearer token auth **not enforced** on `/ui/*` or any five new API endpoints in initial implementation. Auth deferred to future enhancement (see Future Enhancements).

### Persistence

SQLite via `sqlx` with async SQLite driver. Database path configurable (`db.path`; defaults to `/etc/saurron/saurron.db`). `SqlitePool` added to `AppStateInner`. Schema migrations run at startup via `sqlx::migrate!`.

Cycle data written to SQLite at end of every cycle (scheduled and HTTP-triggered) in addition to existing audit log. Web UI reads exclusively from SQLite.

### Styling

**Svelte Material UI (SMUI)** provides MD3-compliant components built on MD3's CSS custom property system (colour roles: `--md-sys-color-primary`, `--md-sys-color-surface-container`, etc.). Dark mode toggled by swapping token values via `class="smui-dark-theme"` on root element; SMUI responds natively. Gives correct MD3 tonal relationships in both themes rather than simple colour inversion.

Tailwind CSS may optionally supplement SMUI for layout utilities (`flex`, `grid`, spacing) but not required. SMUI handles all component-level styling.

---

## Data model

### `cycles` table

| Column         | Type    | Notes                                              |
|----------------|---------|----------------------------------------------------|
| `id`           | INTEGER | Primary key, auto-increment                        |
| `started_at`   | TEXT    | ISO 8601 timestamp                                 |
| `completed_at` | TEXT    | ISO 8601 timestamp                                 |
| `trigger`      | TEXT    | `"scheduled"`, `"manual"`, `"http_api"`            |
| `updated`      | INTEGER | Count of successfully updated containers           |
| `rolled_back`  | INTEGER |                                                    |
| `failed`       | INTEGER |                                                    |
| `skipped`      | INTEGER |                                                    |
| `up_to_date`   | INTEGER |                                                    |

### `cycle_containers` table

| Column      | Type    | Notes                                                           |
|-------------|---------|-----------------------------------------------------------------|
| `id`        | INTEGER | Primary key, auto-increment                                     |
| `cycle_id`  | INTEGER | Foreign key → `cycles.id`                                       |
| `name`      | TEXT    | Container name                                                  |
| `old_image` | TEXT    | Image reference before update (null if up-to-date or skipped)  |
| `new_image` | TEXT    | Image reference after update (null if no update)               |
| `outcome`   | TEXT    | `"updated"`, `"rolled_back"`, `"failed"`, `"skipped"`, `"up_to_date"` |

Record written only after cycle completes. In-progress cycles not visible until finished.

`scanned` count shown in dashboard not stored; computed as `updated + rolled_back + failed + skipped + up_to_date`. Cycle duration not stored; client derives from `completed_at − started_at`.

### `SessionReport` refactoring

`SessionReport` in `update.rs` consolidated to carry rich per-container data:

```rust
pub struct SessionReport {
    pub containers: Vec<ContainerReport>,
}

pub struct ContainerReport {
    pub name: String,
    pub outcome: ContainerOutcome, // Updated | RolledBack | Failed | Skipped | UpToDate
    pub old_image: Option<String>,
    pub new_image: Option<String>,
}
```

Previous `updated: Vec<String>`, `skipped: Vec<String>`, `failed: Vec<String>`, `rolled_back: Vec<String>`, and `up_to_date: usize` fields removed. `render_template` in `notifications.rs` derives equivalent name-lists from `containers` when building MiniJinja context so existing custom templates work unchanged.

`SessionReport::record()` takes additional `old_image: &str` parameter. For `Failed` containers, caller passes container's current image (from `ContainerInfo`); for `Updated` and `RolledBack`, image values come from `UpdateResult` variant directly and parameter is ignored.

---

## Features

### Dashboard (`/ui/`)

Landing page. Gives operators snapshot of Saurron's recent activity.

**Summary strip (MD3 Cards, top of page)**
Three stat cards in a row: *Last cycle* (relative timestamp + outcome badge), *Updates this week* (count), *Failures this week* (count with error colour tint if non-zero).

**Cycle history table (SMUI DataTable)**
Paginated table (20 rows per page). Columns: started at, trigger, scanned, updated, failed, rolled back, duration. Each row expandable to reveal sub-table of per-container outcomes: container name, old image, new image, outcome chip (colour-coded: green = updated, amber = rolled back, red = failed, grey = up-to-date/skipped).

Empty state shown when no cycles recorded yet.

**API:** `GET /v1/history?page=N&per_page=20` → `{ cycles: [...], total: N }`

**API:** `GET /v1/history/:id` → `{ cycle: {...}, containers: [...] }`

---

### Manual Update (`/ui/update`)

Triggers immediate update cycle from browser, equivalent to `POST /v1/update`.

**Filter form (SMUI TextField)**
Two optional inputs — *Container names* (comma-separated) and *Image prefixes* (comma-separated) — matching existing query-parameter semantics of `POST /v1/update`. Both can be blank to target all containers.

**"Run Now" button (MD3 FilledButton)**
Disabled while cycle running. If update lock held, server returns 409 and UI shows SMUI Snackbar: "A cycle is already running."

**Status and results**
After submission, form replaced by SMUI LinearProgress bar while cycle runs. When complete, full response displayed inline in same expandable-row format as dashboard. "Run Another" button resets form.

**API:** `POST /v1/update` (existing endpoint, used directly)

---

### Template Preview (`/ui/template`)

Validates custom MiniJinja notification template against synthetic data before deploying.

**Template editor (SMUI TextField, multiline)**
Pre-filled by calling `GET /v1/template` on load. If response is `null`, editor shows built-in default template as starting point. Monospace font, fills available height.

**Synthetic data selector (MD3 SegmentedButton)**
Three preset scenarios:

| Scenario         | Description                                        |
|------------------|----------------------------------------------------|
| *All updated*    | 3 containers updated, 0 failures                   |
| *Mixed outcomes* | 1 updated, 1 rolled back, 1 failed, 2 up-to-date   |
| *Nothing to do*  | 5 containers, all up-to-date                       |

**"Preview" button (MD3 FilledTonalButton)**
POSTs template string and scenario identifier to server. Server renders template using actual MiniJinja `render_template` with synthetic `SessionReport` for chosen scenario — guarantees preview uses same rendering path as production.

**Preview panel**
Rendered output in read-only `<pre>` block. Template syntax errors appear in MD3 error-coloured banner above panel.

**API:** `GET /v1/template` → `{ template: string | null }` (`null` means no custom template configured; built-in default in effect)

**API:** `POST /v1/template/preview` body: `{ template: string, scenario: string }` → `{ rendered: string }` or `{ error: string }`

---

### Test Notifications (`/ui/notifications`)

Sends real notification to configured targets using synthetic data, to verify delivery end-to-end without waiting for real update cycle.

**Target list (SMUI Cards)**
One card per notification target type (Webhook, Email, MQTT, Pushover). Cards for unconfigured targets shown disabled. Each active card shows target's key identifying field (URL, recipient, broker, etc.) and *"Send Test"* button.

**Scenario selector**
Same three synthetic scenarios as Template Preview; selected scenario applies to all targets.

**Result feedback**
After sending, card shows success or error state inline — green check or red error icon with message. Button re-enables after 3 seconds.

**API:** `POST /v1/notifications/test` body: `{ target: string, scenario: string }` → `{ ok: bool, error?: string }`

Valid `target` values: `"webhook"`, `"email"`, `"mqtt"`, `"pushover"` — matching field names in `NotificationsConfig`. Server returns 400 if named target not configured.

---

## Navigation

SMUI Navigation Drawer (persistent, left side) on desktop; SMUI Bottom App Bar with navigation icons on narrow viewports. Four destinations with Material Symbols icons: Dashboard, Manual Update, Template Preview, Test Notifications.

Drawer header shows status chip indicating whether cycle currently running, updated by polling `GET /v1/health` every 5 seconds while UI open. Health response extended to include `updating: bool` field.

---

## Dark mode

Toggle button in top app bar switches light/dark themes. Preference persisted in `localStorage`, applied before first paint to avoid flash of wrong theme.

SMUI dark mode activated by adding `class="smui-dark-theme"` to `<body>`. SMUI implements MD3 colour roles as CSS custom properties, so toggling class automatically recomputes tonal surface, container, and on-colour values across all components — no per-component style overrides needed.

---

## New configuration

### `http_api.web_ui` (boolean, default `false`)

Enables static file routes for Svelte bundle and five new API endpoints. When `false`, bundle routes and new endpoints not mounted and no SQLite connection opened.

| Layer | Name |
|-------|------|
| TOML  | `[http_api]` section, key `web_ui` |
| CLI   | `--http-api-web-ui` |
| Env   | `SAURRON_HTTP_API_WEB_UI` |

### `db.path` (string, default `/etc/saurron/saurron.db`)

Path to SQLite database file. Created automatically on first run if not exists.

| Layer | Name |
|-------|------|
| TOML  | `[db]` section, key `path` |
| CLI   | `--db-path` |
| Env   | `SAURRON_DB_PATH` |

---

## New dependencies

### Rust (server-side)

| Crate        | Purpose                                                         |
|--------------|-----------------------------------------------------------------|
| `sqlx`       | Async SQLite access, compile-time query checks (offline mode via `sqlx prepare`) |
| `include_dir`| Embed compiled `web/dist/` tree into binary at compile time |

### JavaScript (frontend build)

| Package                | Purpose                                          |
|------------------------|--------------------------------------------------|
| `svelte`               | UI framework                                     |
| `vite` + `@sveltejs/vite-plugin-svelte` | Build toolchain             |
| `svelte-routing`       | Client-side SPA router                           |
| `@smui/*`              | MD3 component packages (Button, Card, DataTable, Drawer, TextField, etc.) |
| `material-symbols`     | MD3 icon font                                    |

---

## Build pipeline

`web/` directory at repository root alongside `src/`. Frontend is separate build step before Rust binary compiled for production Docker images.

```
pnpm install
pnpm build          # Vite + Svelte → web/dist/
cargo sqlx prepare  # update .sqlx/ offline query cache
cargo build --release --features web
```

Compiled `web/dist/` embedded into Rust binary at compile time using `include_dir!`. In development, Vite dev server proxies API requests to locally running Saurron instance — frontend hot-reloads independently of Rust build.

`sqlx` compile-time query checks require `DATABASE_URL` env var pointing to SQLite file when initially authoring queries. Checked-in `.sqlx/` offline query cache (from `cargo sqlx prepare`) used in CI and on developer machines without live database — `DATABASE_URL` not required for normal builds.

`web` Cargo feature gates all SQLite code, static file routes, and five new API endpoints (`/v1/history`, `/v1/history/:id`, `GET /v1/template`, `/v1/template/preview`, `/v1/notifications/test`). `cargo build` without feature produces current minimal binary unchanged, keeping existing Docker image and CI paths unaffected until feature ready.

---

## Alternatives considered

### Leptos (original choice)

Initial framework selection: Rust-native SSR with Axum integration, entire stack in Rust. Blocker was component library ecosystem. Every MD3-oriented library abandoned or unmaintained:

- **leptos-material** — no updates in 2 years
- **leptonic** — no updates in 2 years
- **Thaw UI** — no release in 9 months
- **leptos-shadcn-ui** — open PRs untouched for 5 months
- **Holt** — active but low adoption and incomplete coverage

Proceeding with Leptos meant hand-implementing every MD3 component (Navigation Drawer, Data Table, Chips, Snackbar, etc.) on top of application logic.

### Dioxus

Most actively maintained Rust WASM framework, closest alternative to Leptos. Component library situation similar — sparse and early-stage — same hand-implementation burden. Rejected for same reason as Leptos.

### HTMX + DaisyUI

HTMX handles partial page updates via server-rendered HTML fragments; DaisyUI provides Tailwind-based components. Radically simpler — no build complexity, no WASM toolchain, no separate language — viable for the four features needed.

Not chosen: HTMX provides no UI components. DaisyUI approximates MD3 rather than implementing it; matching MD3 colour roles and component semantics precisely would require same manual CSS work as hand-implementing components in a JS framework, with less benefit since DaisyUI styles would fight MD3 overrides.

### Vue + Vuetify

Vuetify arguably most complete MD3 implementation in any framework. Rejected: Vue adds significant weight and Vuetify bundle size is substantial for an operator-facing admin panel.

### React + MUI

MUI has full MD3 coverage and largest ecosystem. Rejected for same reasons as Vue — too heavyweight.

### Svelte + SMUI (chosen)

Svelte compiles away at build time — small, fast output, no runtime framework. SMUI built directly on MD3's CSS custom property system, giving correct MD3 tonal colour relationships in light and dark themes with clean toggle. Significantly lighter than Vue or React, better MD3 fidelity than any Rust-native option.

---

## Future enhancements

- **Authentication** — Enforce existing `http_api.token` Bearer token on `/ui/*` and five new API endpoints. Add login page prompting for token, stores in `sessionStorage`. Add `401` redirect handling in Svelte fetch wrapper.
- **Live cycle progress** — Replace 5-second health poll with Server-Sent Events for real-time container-by-container progress during running cycle.
- **History search and filtering** — Date range picker, outcome filter, container name search on dashboard table.
- **Notification target configuration UI** — View and edit notification settings from browser rather than editing config files directly.