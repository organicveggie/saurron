# Saurron Web UI — Design Document

## Overview

A server-side-rendered web interface built with Leptos (SSR) and served by Saurron's existing
Axum HTTP server. The UI provides an update history dashboard, manual update triggers, custom
template preview, and test notification dispatch. Styled with Tailwind CSS following Material
Design 3 (MD3) principles.

The web UI is an optional feature, gated by a new `http_api.web_ui` config flag alongside the
existing `http_api.update` and `http_api.metrics` flags.

---

## Architecture

### Rendering model

**Leptos SSR with hydration.** The server renders each page to HTML on request; the WASM bundle
is downloaded by the browser and hydrates the page for interactivity. This avoids a blank loading
state and keeps the initial HTML meaningful for monitoring tools that scrape the UI.

Leptos server functions — `#[server]` annotated async Rust functions — replace REST endpoints for
all UI-driven operations. They run on the server but are callable transparently from component
code.

### Integration with the existing server

The existing `http.rs` Axum router is extended to mount Leptos routes and a server-function
handler. The Leptos `AppState` reuses `AppStateInner` (already in an `Arc`) so the same Docker
client, registry client, and config are shared between the REST API and the web UI without
duplication.

```
Axum router
├── /v1/update          (existing REST)
├── /v1/health          (existing REST)
├── /v1/metrics         (existing REST)
├── /api/leptos         (server-function handler — new)
├── /pkg/*              (static WASM + JS assets — new)
└── /*                  (Leptos SSR routes — new)
    ├── /               Dashboard
    ├── /update         Manual Update
    ├── /template       Template Preview
    └── /notifications  Test Notifications
```

### Persistence

SQLite via `sqlx` with the async SQLite driver. The database path is configurable
(`db.path`; defaults to `/etc/saurron/saurron.db`). A connection pool (`SqlitePool`) is added to
`AppStateInner`. Schema migrations run at startup via `sqlx::migrate!`.

Cycle data is written to SQLite at the end of every cycle (both scheduled and HTTP-triggered) in
addition to the existing audit log. The web UI reads exclusively from SQLite; it does not parse
the audit log file.

### Styling

Tailwind CSS for utility classes, configured with MD3 design tokens (colour roles, typescale,
shape scale, elevation). The `material-tailwind` npm package provides ready-made MD3-compatible
component styles as a Tailwind plugin, reducing the amount of custom CSS. A single
`style/input.css` is processed by the Tailwind CLI during the build.

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

A database record is written only after a cycle completes. In-progress cycles are not visible in
the UI until they finish.

---

## Features

### Dashboard (`/`)

The landing page. Gives operators a snapshot of Saurron's recent activity.

**Summary strip (MD3 Cards, top of page)**
Three stat cards in a row: *Last cycle* (relative timestamp + outcome badge), *Updates this
week* (count), *Failures this week* (count with red tint if non-zero).

**Cycle history table**
Paginated table (20 rows per page, MD3 DataTable style). Columns: started at, trigger,
scanned, updated, failed, rolled back, duration. Each row is expandable to reveal a sub-table
of per-container outcomes for that cycle. The expanded view shows container name, old image,
new image, and an outcome chip (colour-coded: green = updated, amber = rolled back, red =
failed, grey = up-to-date / skipped).

Empty state shown when no cycles have been recorded yet.

**Server function:** `get_history(page: u32) -> Vec<CycleSummary>`

**Server function:** `get_cycle_detail(id: i64) -> Vec<ContainerResult>`

---

### Manual Update (`/update`)

Triggers an immediate update cycle from the browser, equivalent to `POST /v1/update` but with
a UI.

**Filter form (MD3 outlined text fields)**
Two optional inputs — *Container names* (comma-separated) and *Image prefixes*
(comma-separated) — matching the existing query-parameter semantics. Both fields can be left
blank to target all containers.

**"Run Now" button (MD3 FilledButton)**
Disabled while a cycle is already running (the existing update lock prevents concurrent cycles;
the server function returns an appropriate error if the lock is held, displayed as an MD3
Snackbar).

**Status and results**
After the button is clicked, the form is replaced by a progress indicator (MD3 LinearProgressBar)
while the cycle runs. When complete, the full `SessionReport` is displayed inline in the same
expandable-row format used by the dashboard history table. A "Run Another" button resets the
form.

**Server function:** `trigger_update(containers: Option<Vec<String>>, images: Option<Vec<String>>) -> Result<SessionReport, String>`

---

### Template Preview (`/template`)

Lets operators validate a custom MiniJinja notification template against synthetic data before
deploying it.

**Template editor (MD3 outlined textarea)**
Pre-filled with the currently configured template, or the built-in default if none is set. Full
height of the viewport minus the toolbar. Monospace font.

**Synthetic data selector (MD3 SegmentedButton)**
Three preset scenarios the operator can choose from:

| Scenario         | Description                                        |
|------------------|----------------------------------------------------|
| *All updated*    | 3 containers updated, 0 failures                   |
| *Mixed outcomes* | 1 updated, 1 rolled back, 1 failed, 2 up-to-date   |
| *Nothing to do*  | 5 containers, all up-to-date                       |

**"Preview" button (MD3 FilledTonalButton)**
Sends the template string and selected scenario identifier to the server. The server renders the
template using the actual MiniJinja `render_template` function with the synthetic `SessionReport`
populated for the chosen scenario. This guarantees the preview reflects the exact same rendering
path used in production.

**Preview panel**
Rendered output shown in a read-only `<pre>` block below the editor. Error messages (template
syntax errors) are shown in an MD3 error-coloured banner above the preview panel rather than
replacing the output.

**Server function:** `preview_template(template: String, scenario: PreviewScenario) -> Result<String, String>`

---

### Test Notifications (`/notifications`)

Sends a real notification to one or more configured targets using synthetic data, so operators
can verify delivery end-to-end without waiting for a real update cycle.

**Target list**
One MD3 Card per configured notification target (Webhook, Email, MQTT, Pushover). Cards for
unconfigured targets are shown in a disabled/greyed state with a message indicating the target
is not set up. Each active card shows the target's key identifying field (webhook URL, email
recipient, MQTT broker, etc.) and a *"Send Test"* FilledButton.

**Scenario selector**
Same three synthetic scenarios as the Template Preview page; selected scenario applies to all
targets.

**Result feedback**
After sending, the card shows a success or error state inline — a green check icon with
"Delivered" or a red error icon with the error message. The button re-enables after 3 seconds
to allow re-sending.

Notifications are sent using the existing `send_webhook`, `send_email`, `send_mqtt`, and
`send_pushover` functions directly — no new sending logic required.

**Server function:** `send_test_notification(target: NotificationTarget, scenario: PreviewScenario) -> Result<(), String>`

---

## Navigation

MD3 Navigation Drawer (persistent, left-side) on desktop viewports; MD3 Navigation Bar (bottom)
on narrow viewports. Four destinations: Dashboard, Manual Update, Template Preview, Test
Notifications. Icons from the Material Symbols set (filled style).

The drawer also shows a read-only status chip indicating whether a cycle is currently running,
updated by a lightweight polling server function called every 5 seconds when the UI is open.

---

## New configuration

### `http_api.web_ui` (boolean, default `false`)

Enables the web UI routes and the SQLite history store. When `false`, no Leptos routes or the
server-function handler are mounted, and no SQLite connection is opened — identical behaviour to
the current binary.

### `db.path` (string, default `/etc/saurron/saurron.db`)

Path to the SQLite database file. Created automatically on first run if it does not exist.

---

## New dependencies

| Crate / Package       | Purpose                                              |
|-----------------------|------------------------------------------------------|
| `leptos`              | UI framework (SSR feature)                           |
| `leptos_axum`         | Axum integration for Leptos                          |
| `leptos_meta`         | `<head>` management (title, meta tags)               |
| `leptos_router`       | Client-side routing with SSR support                 |
| `sqlx`                | Async SQLite access, compile-time query checking     |
| `tailwindcss` (npm)   | CSS utility framework                                |
| `material-tailwind`   | MD3 component styles as a Tailwind plugin            |
| `cargo-leptos`        | Build toolchain (server + WASM + Tailwind)           |

---

## Build pipeline

`cargo-leptos` replaces the direct `cargo build` call for producing the web-enabled binary. It:

1. Compiles the server binary (standard Rust target).
2. Compiles the WASM bundle (via `wasm-bindgen`).
3. Runs the Tailwind CLI to generate `style/output.css`.
4. Outputs everything to `target/site/`.

The existing `cargo build` path (for the binary without the web UI) remains functional; the web
UI is compiled in only when the `web` Cargo feature is enabled. This keeps the minimal deployment
story intact.

The Docker workflow and GitHub Actions CI will need a new step to install `cargo-leptos` and the
Tailwind CLI when building with the web feature.

---

## Future enhancements

- **Authentication** — Bearer token prompt on first visit, stored in `sessionStorage`. The
  existing `http_api.token` config value covers the web UI without a separate token. Add login
  UX and `401` redirect handling.
- **Live cycle status** — Replace the 5-second polling status chip with Server-Sent Events
  for real-time progress during a running cycle.
- **History search and filtering** — Date range picker, outcome filter, container name search
  on the dashboard table.
- **Notification target configuration UI** — View and edit notification settings from the browser
  rather than editing config files directly.
- **Dark mode** — MD3 dynamic colour with a light/dark toggle, persisted in `localStorage`.
