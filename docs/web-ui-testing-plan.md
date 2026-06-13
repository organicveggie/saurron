# Web UI Testing Plan

## Goal

Bring `web/` from **zero tests** to **≥ 50% coverage**, following
[`docs/claude/testing.md`](claude/testing.md): Vitest + Playwright-backed Browser Mode
(`vitest-browser-svelte`), MSW for network mocking, ARIA-role locators, no simulated DOM.

Note: codebase is plain JS (no TypeScript), so test files use `.test.js` /
`.svelte.test.js` — the doc's `.test.ts` examples translate directly.

## Current state

No `vitest`, no test files, no coverage tooling in `web/package.json`. 22 source files,
~2080 lines (markup + script + style). Breakdown by weight:

| Group | Files | Approx. lines |
|---|---|---|
| Routes | `Dashboard` (462), `Update`/`Template`/`Notifications` (1 each) | ~465 |
| Nav chrome | `NavDrawer` (360), `TimelineEvent` (265), `TopAppBar` (117), `BottomNavBar` (77) | ~820 |
| Molecules | `CycleStatusCard` (134), `FilterDropdown` (131), `ContainerOutcomeRow` (114), `StatCard` (82), `Sparkline` (75) | ~535 |
| Atoms | `OutcomeChip`, `TriggerChip`, `RunningChip` | ~60 |
| `lib/*.js` | `time.js` (84), `api.js` (23), `image.js` (13) | ~120 |
| `stores/*.js` | `health.js` (36), `theme.js` (9), `search.js` (2) | ~47 |
| `App.svelte`, `main.js` | | ~87 |

Pure-JS + stores + atoms + molecules alone covers well over half the instrumentable
script lines, so milestones M2–M4 should already approach the 50% target; M5–M6 build in
margin and cover the highest-traffic UI surfaces.

---

## Tooling decisions

- **Test runner**: Vitest, split into two projects/environments via workspace config:
  - `unit` — Node environment, for `lib/*.js` and pure-logic stores (`*.test.js`)
  - `browser` — Playwright/Chromium Browser Mode via `vitest-browser-svelte` and
    `@vitest/browser/context`, for `*.svelte` components (`*.svelte.test.js`) **and**
    any store that touches the DOM directly (`*.test.js` placed in the browser project)
- **Coverage provider**: `@vitest/coverage-v8` (works with the Chromium/Playwright
  provider via CDP; no separate instrumentation plugin needed). Fall back to
  `@vitest/coverage-istanbul` if v8 browser-mode coverage proves flaky.
- **Network mocking**: MSW (`msw`) with handlers for `/v1/health`, `/v1/history`,
  `/v1/history/:id`, `/v1/containers` — reused across store and route tests.
- **New devDependencies**: `vitest`, `@vitest/browser`, `@vitest/coverage-v8`,
  `vitest-browser-svelte`, `playwright` (+ `npx playwright install chromium`), `msw`.
- **Scripts** added to `web/package.json`:
  - `test` — run unit + browser projects once
  - `test:watch` — watch mode for local dev
  - `test:coverage` — run with coverage, text + lcov + html reporters

---

## Milestone 1 — Test infrastructure (no coverage yet)

**Commit**: `test(web): add Vitest + Playwright browser-mode test infrastructure`

- Add devDependencies and `pnpm` scripts listed above.
- `vitest.config.js` (or `vitest.workspace.js`) defining `unit` (node) and `browser`
  (chromium via `vitest-browser-svelte`) projects, matching the structure shown in
  `docs/claude/testing.md` §3.
- `src/test/msw/handlers.js` + `src/test/msw/server.js` (or browser worker) with mock
  fixtures for the four `/v1/*` endpoints, fed by a small set of representative fixture
  objects (`src/test/fixtures/cycles.js`, `containers.js`, `health.js`).
- One smoke test in each project (`time.test.js` trivial assertion, a one-line
  `RunningChip.svelte.test.js` render check) to prove the pipeline runs end to end —
  superseded by real tests in M2/M3.
- Update `CLAUDE.md` "Frontend" command block with `pnpm test` / `pnpm test:coverage`.

## Milestone 2 — Pure JS: `lib/*.js` and `stores/*.js`

**Commit**: `test(web): cover lib utilities and stores`

Node-environment tests, no DOM:

- `time.test.js`: `formatRelative` (just-now/minutes/hours/days boundaries),
  `formatAbs`, `formatDuration` (sub-minute, exact-minute, minute+seconds),
  `formatTime`, `groupByDay` (Today/Yesterday/weekday/full-date label branches,
  grouping/ordering), `getDailyAggregates` (7-day bucket, summation, empty input).
- `image.test.js`: `imageTagOnly` (no ref, digest-suffixed, registry-prefixed paths),
  `imageShortDigest` (no ref, no digest, present digest — slice offsets).
- `api.test.js`: each exported function builds the right URL/query string and
  surfaces non-OK responses as thrown errors — mock global `fetch` with `vi.fn()`.
- `stores/health.test.js`: mock global `fetch` (or MSW) for `/v1/health` *before*
  importing the module — `poll()` runs as a load-time side effect (it isn't
  exported), so assert the populated store after a fresh `import()`. Cover `??`
  fallbacks for missing response fields and swallowed network errors (rejected
  fetch → store keeps last-known value). Use `vi.useFakeTimers()` +
  `vi.advanceTimersByTimeAsync(5000)` to verify the `setInterval(poll, 5000)`
  re-fetch, and `vi.resetModules()` between cases to avoid interval leakage
  (see "Open items").
- `stores/search.test.js`, `stores/theme.test.js`: **run in the `browser` project**,
  not `unit` — `theme.js` reads `document.documentElement.dataset.theme` at
  import time and its `subscribe` callback writes `data-theme` + `localStorage`
  back to the real `document` (this *is* the store's job: it drives CSS theming
  via `[data-theme]` selectors, mirroring the anti-flash inline script in
  `index.html`). Real Chromium DOM via Browser Mode covers this without adding
  a `jsdom`/`happy-dom` dependency. `search.js` has no DOM dependency but is
  small enough to colocate here for consistency. Assert: store initial value,
  `theme` toggling writes both `data-theme` attribute and `localStorage`.

## Milestone 3 — Atom components

**Commit**: `test(web): cover atom components (chips)`

Browser-mode, ARIA-locator-driven:

- `OutcomeChip.svelte.test.js`: each `outcome` value renders its icon/label/class;
  unknown outcome falls back to `up_to_date`; `dense` toggles inline sizing.
- `TriggerChip.svelte.test.js`: each `trigger` value renders its icon/label; unknown
  falls back to `scheduled`.
- `RunningChip.svelte.test.js`: `running` true/false toggles label and `live`/`idle`
  class.

## Milestone 4 — Molecule components

**Commit**: `test(web): cover stat/timeline molecule components`

- `StatCard.svelte.test.js`: renders `label`/`value`/`sub`; `tone="error"` swaps
  container colors; `icon`, `badge` snippet, `chart` snippet render conditionally.
- `Sparkline.svelte.test.js`: point count matches `data.length`; empty data renders no
  `<svg>`; single-point centers at `W/2`; `bad` dots render only when
  `failed`/`rolled_back` present; `polyline`/`area` path strings well-formed.
- `FilterDropdown.svelte.test.js`: click opens/closes dropdown, selecting an option
  updates bound `value` and closes, outside-click (`mousedown` on document) closes,
  active option shows check icon.
- `ContainerOutcomeRow.svelte.test.js`: renders name, old/new image tag + short digest,
  arrow separator, falls back to `reason` when `new_image` is absent, dot color maps
  per `outcome` (including unknown → neutral).
- `CycleStatusCard.svelte.test.js`: idle branch renders countdown/schedule label for
  each `schedule_mode` (`run_once`/`interval` at various second values/`cron`/unknown);
  running branch renders elapsed timer (fake timers + `$tick`), `phase`/`pct`/`current`/
  `count`. Mock the `health` store import.

**M4 implementation decisions:**

- **StatCard snippet props** — `badge`/`chart` are Svelte 5 snippet props; plain `.test.js`
  cannot define snippets. Solution: `src/test/wrappers/StatCardWithSnippets.svelte` passes
  hardcoded `<span class="test-badge">` / `<span class="test-chart">` snippets and forwards
  all other props via `{...props}`. Absence tests use `StatCard` directly (no wrapper).
- **FilterDropdown outside-click** — `await userEvent.click(document.body)` (imported from
  `@vitest/browser/context`) fires `mousedown` on `<body>`; `wrapEl.contains(body)` is
  `false` so the document handler sets `open = false`.
- **CycleStatusCard `health` mock** — `vi.hoisted()` creates a minimal hand-rolled store
  (subscribe/set) before module mocks run; `vi.mock('../stores/health.js', () => ({ health:
  mockHealth }))` uses that reference. `vi.useFakeTimers()` + `vi.setSystemTime()` control
  `Date.now()` for elapsed/countdown assertions. For idle tests `next_run_at: null` (→ `—`)
  avoids time-sensitive countdown checks; one dedicated test sets a specific fake time.

## Milestone 5 — Navigation chrome

**Commit**: `test(web): cover navigation chrome components`

- `TopAppBar.svelte.test.js`: title/subtitle render; theme toggle button flips
  `$theme` and icon; search input two-way-binds `$searchQuery`; `Cmd/Ctrl+K` focuses
  search field; `Run cycle` button vs disabled `Running…` state driven by
  `$health.updating`; progress underline appears when updating.
- `BottomNavBar.svelte.test.js`: each nav item renders, active route gets `.active`
  class + filled icon, click calls `push` with the right route (mock
  `svelte-spa-router`).
- `NavDrawer.svelte.test.js`: `rail` vs `standard` variant render branches; nav item
  active-state highlighting; watched-container list population from mocked
  `getContainers`, preview/`show more`/`show less` toggle at the `PREVIEW_COUNT`
  boundary, `state-dot` color mapping (`running`/`monitor_only`/other), `monitor_only`
  vs `pinned` icon display; cycle badge running vs idle.
- `TimelineEvent.svelte.test.js`: duration computation (with/without
  `completed_at`), `isQuiet` branch (all-zero outcome counts) vs active branch text
  composition (updated/rolled-back/failed combinations and comma placement),
  `activeDotColor` precedence (`failed` > `rolled_back` > `updated` > neutral),
  preview chips show only when collapsed and non-quiet, expand/collapse toggles
  `expanded-panel` and calls `onToggle` (click + Enter/Space keydown).

## Milestone 6 — Routes

**Commit**: `test(web): cover route components`

- `Dashboard.svelte.test.js` (the big one — mock `getHistory` via MSW handlers/
  fixtures):
  - Loading → populated → empty-state → error-fallback (`catch` branch) renders
  - Stat-card derivations: `lastCycle*` fields, `updatesThisWeek`/`failuresThisWeek`/
    `cyclesThisWeek` arithmetic, `noData` em-dash fallback, `failuresTone`/`failuresIcon`
    branch flip at `failuresThisWeek > 0`
  - Filter pipeline: time-window cutoffs, trigger mapping, segment filters
    (`Updates`/`Failures`/`All`), search-query name matching — each combinable
  - `dayGroups` rendering grouped by `groupByDay`, group metadata line
    (singular/plural "cycle", incident suffix)
  - `Load older` button: appears when `hasMore`, calls `getHistory` with next page,
    appends results, disables while `loadingMore`
  - `filtersActive` flips empty-state messaging between "no cycles yet" and
    "no cycles match filters"
- `Update.svelte.test.js`, `Template.svelte.test.js`, `Notifications.svelte.test.js`:
  placeholder smoke tests asserting the stub text renders — cheap coverage, and they'll
  need real tests once these routes grow beyond placeholders (tracked separately, not
  in this plan's scope).
- `App.svelte.test.js` (optional/stretch — only if needed to clear 50%): drawer variant
  switches at the 600px/900px breakpoints on `resize`, route → page-title mapping,
  `NavDrawer` hidden below 600px.

## Milestone 7 — CI integration

**Commit**: `ci(web): run tests and enforce coverage threshold`

- New `.github/workflows/web-test.yml`, mirroring `web-lint.yml`'s trigger paths
  (`web/src/**/*.svelte`, `web/src/**/*.js`, `web/package.json`, `web/pnpm-lock.yaml`)
  and `pnpm/action-setup` + `actions/setup-node` steps.
- Steps: `pnpm install --frozen-lockfile` → `npx playwright install --with-deps
  chromium` → `pnpm test:coverage` → upload lcov/html artifact (matching the Rust
  `coverage.yml` pattern) → fail the job if total coverage < 50%.
- Update `docs/claude/testing.md` if any deviations from the documented approach
  emerged during implementation (e.g. coverage-provider fallback).

---

## Open items / things to confirm as we go

- If `@vitest/coverage-v8` proves unreliable in Browser Mode (known rough edges with
  CDP-based coverage + Playwright), swap to `@vitest/coverage-istanbul` — small config
  change, called out in M1.
- `health.js` runs `poll()` and `setInterval` at module load — tests need to either
  mock `fetch` before import or use `vi.useFakeTimers()` + module reset between cases
  to avoid cross-test interval leakage.
- `NavDrawer`/`TopAppBar` import the shared `health`/`theme`/`searchQuery` stores
  directly (not as props) — tests will need to mutate the store value directly
  (`store.set(...)`) rather than passing props, and reset it afterward.
