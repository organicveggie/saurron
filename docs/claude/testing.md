# Code Testing

## Rust

- Coverage target: ≥ 50%
- Unit tests inline each module under `#[cfg(test)] mod tests`; run `cargo test` (fast, no Docker need).
- Integration tests in `tests/integration.rs`; all marked `#[ignore]` — run `cargo test --test integration -- --include-ignored` (need live Docker socket, pulls `registry:2`, `busybox`, `alpine` from Docker Hub).

## Javascript / Typescript

- Coverage target: ≥ 50%

### 1. Tooling

* **Unit & Component Testing:** [Vitest](https://vitest.dev/)
* **End-to-End (E2E) Testing:** [Playwright](https://playwright.dev/)

### 2. Philosophy

JSDOM/Happy DOM fast for pure JS utilities, but miss real CSS layout bugs, complex focus management, accurate event handling.

Avoid simulated DOMs, prefer Vitest Browser Mode (powered by Playwright or WebdriverIO under hood) using `@vitest/browser/context` and `vitest-browser-svelte`.

* Test logic, utilities, pure functions (`.test.ts`) in standard Node environment
* Test Svelte components (`.svelte.test.ts`) inside Browser Mode

### 3. Svelte 5 Runes Reactivity in Tests

* **Component-Internal State:** State living entirely inside rendered Svelte component works automatic during user interactions in tests.
* **Universal/External State:** Testing global/shared reactive state imported from external `.svelte.ts` file — updates won't auto-trigger DOM updates in test env. Wrap mutations in `flushSync()`:

```ts
import { flushSync } from 'svelte';
import { page } from '@vitest/browser/context';
import { myGlobalState } from './store.svelte.ts';

it('updates UI when global state changes', async () => {
  // Update state inside flushSync to force Svelte to update the DOM immediately
  flushSync(() => {
    myGlobalState.count += 1;
  });

  // Now the DOM assertion will succeed safely
  await expect.element(page.getByText('Count is 1')).toBeVisible();
});
```

### 4. Test User Behaviors, Not Implementation Details

Tests should interact with component exactly like real user or assistive tech would.

#### Use ARIA Locators and Roles

Instead of targeting internal CSS classes, test IDs, structural elements (like `div`), use accessible roles. Guarantees code stays accessible, tests don't break during HTML/CSS refactors.

```ts
// ❌ Avoid: Highly coupled to implementation
await page.element('.btn-submit-active').click();

//  Good: Tests what the user actually experiences
await page.getByRole('button', { name: /submit/i }).click();
```

### Avoid Testing Internal Component State

Don't write assertions against component's internal variables or private methods. Assert against its "public contract":

* **Inputs (Props):** Does passing specific prop change what's visible/accessible?
* **Outputs (Events/DOM changes):** Does clicking button trigger expected event or DOM change?

### Cleanly Mock External Dependencies

Isolate component tests from network requests, heavy third-party packages — keeps them fast.

* **API/Network Mocking:** Use MSW (Mock Service Worker). Intercepts requests at network level rather than mocking individual fetch functions — compatible with browser mode and SSR environments.
* **Module Mocking:** Use Vitest's inline `vi.mock()` syntax with explicit paths — works natively across Vite's module resolution:

```ts
vi.mock(import('./utils/analytics.js'), () => ({
  trackEvent: vi.fn(),
}));
```

### Structure Configurations

Keep configs modular. Instead of cluttering production `vite.config.ts`, use Vitest's project workspace definitions or conditional config so testing deps don't bleed into production bundles.

Clean approach for `vitest.config.ts` dedicated to split environments:

```ts
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  test: {
    // Configures tests using real browsers
    browser: {
      enabled: true,
      name: 'chromium', // or 'firefox', 'webkit'
      provider: 'playwright',
    },
  },
});
```
