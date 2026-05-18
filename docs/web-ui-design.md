# Saurron Web UI — Design Document

## Overview

A single-page application (SPA) built with Svelte and styled with Svelte Material UI (SMUI),
served as a static asset bundle by Saurron's existing Axum HTTP server. The UI provides an
update history dashboard, manual update triggers, custom template preview, and test notification
dispatch, following Material Design 3 (MD3) principles including light/dark mode switching.

The web UI is an optional feature, gated by a new `http_api.web_ui` config flag alongside the
existing `http_api.update` and `http_api.metrics` flags.

---

## Architecture

### Rendering model

**Svelte SPA (client-side rendering).** Svelte compiles to a small, optimised JS/CSS bundle
with no runtime framework overhead. Axum serves the bundle as static files; the browser
downloads it once and handles all subsequent navigation client-side. The SPA calls Saurron's
HTTP API for all data.

SvelteKit SSR was considered and rejected: it requires a persistent Node.js server process in
production, which complicates deployment and introduces a second runtime dependency. Svelte's
compiled output renders fast enough on the client that the blank-loading-state concern that
motivates SSR is not material for an operator dashboard with modest data volumes.

### Integration with the existing server

Axum serves the compiled Svelte bundle from a `/ui` path when `http_api.web_ui` is enabled.
The SPA communicates with Saurron exclusively through the HTTP API. Four new REST endpoints are
added alongside the existing three:

```
Axum router
├── /v1/update                   (existing)
├── /v1/health                   (existing)
├── /v1/metrics                  (existing)
├── /v1/history                  (new — paginated cycle list)
├── /v1/history/:id              (new — single cycle with per-container detail)
├── /v1/template/preview         (new — render template with synthetic data)
├── /v1/notifications/test       (new — send test notification to a target)
└── /ui/*                        (new — static SPA bundle)
```

All new endpoints respect the existing Bearer token auth when `http_api.token` is set. The SPA
stores the token in `sessionStorage` and attaches it as an `Authorization: Bearer` header on
every request (see Future Enhancements for the login UX).

### Persistence

SQLite via `sqlx` with the async SQLite driver. The database path is configurable
(`db.path`; defaults to `/etc/saurron/saurron.db`). A connection pool (`SqlitePool`) is added
to `AppStateInner`. Schema migrations run at startup via `sqlx::migrate!`.

Cycle data is written to SQLite at the end of every cycle (both scheduled and HTTP-triggered)
in addition to the existing audit log. The web UI reads exclusively from SQLite.

### Styling

**Svelte Material UI (SMUI)** provides MD3-compliant components built on MD3's CSS custom
property system (colour roles such as `--md-sys-color-primary`,
`--md-sys-color-surface-container`, etc.). Dark mode is toggled by swapping the token values
via a `class="smui-dark-theme"` attribute on the root element, which SMUI responds to natively.
This gives correct MD3 tonal relationships in both themes rather than simple colour inversion.

Tailwind CSS may optionally supplement SMUI for layout utilities (`flex`, `grid`, spacing) but
is not required. SMUI handles all component-level styling.

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

A record is written only after a cycle completes. In-progress cycles are not visible in the UI
until they finish.

---

## Features

### Dashboard (`/ui/`)

The landing page. Gives operators a snapshot of Saurron's recent activity.

**Summary strip (MD3 Cards, top of page)**
Three stat cards in a row: *Last cycle* (relative timestamp + outcome badge), *Updates this
week* (count), *Failures this week* (count with error colour tint if non-zero).

**Cycle history table (SMUI DataTable)**
Paginated table (20 rows per page). Columns: started at, trigger, scanned, updated, failed,
rolled back, duration. Each row is expandable to reveal a sub-table of per-container outcomes
for that cycle, showing container name, old image, new image, and an outcome chip
(colour-coded: green = updated, amber = rolled back, red = failed, grey = up-to-date/skipped).

Empty state shown when no cycles have been recorded yet.

**API:** `GET /v1/history?page=N&per_page=20` → `{ cycles: [...], total: N }`

**API:** `GET /v1/history/:id` → `{ cycle: {...}, containers: [...] }`

---

### Manual Update (`/ui/update`)

Triggers an immediate update cycle from the browser, equivalent to `POST /v1/update`.

**Filter form (SMUI TextField)**
Two optional inputs — *Container names* (comma-separated) and *Image prefixes*
(comma-separated) — matching the existing query-parameter semantics of `POST /v1/update`.
Both can be left blank to target all containers.

**"Run Now" button (MD3 FilledButton)**
Disabled while a cycle is running. If the update lock is held, the server returns 409 and
the UI shows an SMUI Snackbar: "A cycle is already running."

**Status and results**
After submission, the form is replaced by an SMUI LinearProgress bar while the cycle runs.
When complete, the full response is displayed inline in the same expandable-row format used
by the dashboard. A "Run Another" button resets the form.

**API:** `POST /v1/update` (existing endpoint, used directly)

---

### Template Preview (`/ui/template`)

Validates a custom MiniJinja notification template against synthetic data before deploying it.

**Template editor (SMUI TextField, multiline)**
Pre-filled with the currently configured template, or the built-in default if none is set.
Monospace font. Fills the available height.

**Synthetic data selector (MD3 SegmentedButton)**
Three preset scenarios:

| Scenario         | Description                                        |
|------------------|----------------------------------------------------|
| *All updated*    | 3 containers updated, 0 failures                   |
| *Mixed outcomes* | 1 updated, 1 rolled back, 1 failed, 2 up-to-date   |
| *Nothing to do*  | 5 containers, all up-to-date                       |

**"Preview" button (MD3 FilledTonalButton)**
POSTs the template string and scenario identifier to the server. The server renders the
template using the actual MiniJinja `render_template` function with a synthetic `SessionReport`
for the chosen scenario, guaranteeing the preview uses the same rendering path as production.

**Preview panel**
Rendered output in a read-only `<pre>` block. Template syntax errors appear in an MD3
error-coloured banner above the panel.

**API:** `POST /v1/template/preview` body: `{ template: string, scenario: string }` → `{ rendered: string }` or `{ error: string }`

---

### Test Notifications (`/ui/notifications`)

Sends a real notification to configured targets using synthetic data, to verify delivery
end-to-end without waiting for a real update cycle.

**Target list (SMUI Cards)**
One card per notification target type (Webhook, Email, MQTT, Pushover). Cards for unconfigured
targets are shown in a disabled state. Each active card shows the target's key identifying
field (URL, recipient, broker, etc.) and a *"Send Test"* button.

**Scenario selector**
Same three synthetic scenarios as the Template Preview page; the selected scenario applies to
all targets.

**Result feedback**
After sending, the card shows a success or error state inline — a green check icon or a red
error icon with the message. The button re-enables after 3 seconds.

**API:** `POST /v1/notifications/test` body: `{ target: string, scenario: string }` → `{ ok: bool, error?: string }`

---

## Navigation

SMUI Navigation Drawer (persistent, left side) on desktop viewports; SMUI Bottom App Bar with
navigation icons on narrow viewports. Four destinations with Material Symbols icons:
Dashboard, Manual Update, Template Preview, Test Notifications.

The drawer header shows a status chip indicating whether a cycle is currently running, updated
by polling `GET /v1/health` every 5 seconds while the UI is open. The health response is
extended to include an `updating: bool` field.

---

## Dark mode

A toggle button in the top app bar switches between light and dark themes. The preference is
persisted in `localStorage` and applied before first paint to avoid a flash of the wrong theme.

SMUI's dark mode is activated by adding `class="smui-dark-theme"` to the `<body>` element.
Because SMUI implements MD3 colour roles as CSS custom properties, toggling the class
automatically recomputes tonal surface, container, and on-colour values across all components
without any per-component style overrides.

---

## New configuration

### `http_api.web_ui` (boolean, default `false`)

Enables the static file routes for the Svelte bundle and the four new API endpoints. When
`false`, the bundle routes and new endpoints are not mounted and no SQLite connection is
opened.

### `db.path` (string, default `/etc/saurron/saurron.db`)

Path to the SQLite database file. Created automatically on first run if it does not exist.

---

## New dependencies

### Rust (server-side)

| Crate    | Purpose                                         |
|----------|-------------------------------------------------|
| `sqlx`   | Async SQLite access, compile-time query checks  |

### JavaScript (frontend build)

| Package                | Purpose                                          |
|------------------------|--------------------------------------------------|
| `svelte`               | UI framework                                     |
| `vite` + `@sveltejs/vite-plugin-svelte` | Build toolchain             |
| `@smui/*`              | MD3 component packages (Button, Card, DataTable, Drawer, TextField, etc.) |
| `material-symbols`     | MD3 icon font                                    |

---

## Build pipeline

The frontend is a separate build step that runs before the Rust binary is compiled for
production Docker images.

```
pnpm install
pnpm build          # Vite + Svelte → ui/dist/
cargo build --release --features web
```

The compiled `ui/dist/` directory is embedded into the Rust binary at compile time using
`include_dir!` (or served from a path configured at runtime). In development, the Vite dev
server proxies API requests to a locally running Saurron instance, so the frontend hot-reloads
independently of the Rust build.

The `web` Cargo feature gates all SQLite and static-file-serving code. `cargo build` without
the feature produces the current minimal binary unchanged, keeping the existing Docker image
and CI paths unaffected until the feature is ready to ship.

---

## Alternatives considered

### Leptos (original choice)

Leptos was the initial framework selection: a Rust-native SSR framework with Axum integration
that would have kept the entire stack in Rust. The blocker was the Leptos component library
ecosystem. Every MD3-oriented library was abandoned or unmaintained:

- **leptos-material** — no updates in 2 years
- **leptonic** — no updates in 2 years
- **Thaw UI** — no release in 9 months
- **leptos-shadcn-ui** — open PRs untouched for 5 months
- **Holt** — active but low adoption and incomplete coverage

Proceeding with Leptos would have meant hand-implementing every MD3 component (Navigation
Drawer, Data Table, Chips, Snackbar, etc.) in addition to the application logic.

### Dioxus

The most actively maintained Rust WASM framework and the closest direct alternative to Leptos.
The component library situation is similar — sparse and early-stage — so the same
hand-implementation burden would apply. Rejected for the same reason as Leptos.

### HTMX + DaisyUI

HTMX handles partial page updates via server-rendered HTML fragments; DaisyUI provides
Tailwind-based UI components. The approach is radically simpler than any WASM framework —
no build complexity, no WASM toolchain, no separate language — and viable for the four
features this UI needs.

The reason it was not chosen: HTMX itself provides no UI components. DaisyUI has a light/dark
mode story but approximates MD3 rather than implementing it; matching MD3 colour roles and
component semantics precisely would require the same manual CSS work as hand-implementing
components in a JS framework, with less benefit since DaisyUI's own component styles would
be fighting the MD3 overrides.

### Vue + Vuetify

Vuetify is arguably the most complete MD3 implementation available in any framework. Rejected
because Vue adds significant weight to the frontend stack and Vuetify's bundle size is
substantial for an operator-facing admin panel.

### React + MUI

MUI has full MD3 coverage and the largest ecosystem. Rejected for the same reasons as Vue —
too heavyweight for this use case.

### Svelte + SMUI (chosen)

Svelte compiles away at build time, producing small and fast output with no runtime framework.
SMUI is built directly on MD3's CSS custom property system, giving correct MD3 tonal colour
relationships in both light and dark themes with a clean toggle mechanism. The combination
is significantly lighter than Vue or React while providing better MD3 fidelity than any
available Rust-native option.

---

## Future enhancements

- **Authentication UX** — Login page prompting for the Bearer token, stored in `sessionStorage`.
  The existing `http_api.token` config value covers the web UI without a separate credential.
  Add `401` redirect handling in the Svelte fetch wrapper.
- **Live cycle progress** — Replace the 5-second health poll with Server-Sent Events for
  real-time container-by-container progress during a running cycle.
- **History search and filtering** — Date range picker, outcome filter, container name search
  on the dashboard table.
- **Notification target configuration UI** — View and edit notification settings from the
  browser rather than editing config files directly.
