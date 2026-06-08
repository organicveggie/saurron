import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { playwright } from '@vitest/browser-playwright';

export default defineConfig({
  plugins: [svelte()],
  test: {
    projects: [
      {
        extends: true,
        test: {
          name: 'unit',
          environment: 'node',
          include: ['src/**/*.test.js'],
          exclude: ['src/**/*.svelte.test.js', 'src/stores/{search,theme}.test.js'],
          setupFiles: ['./src/test/msw/setup-node.js'],
        },
      },
      {
        extends: true,
        test: {
          name: 'browser',
          include: ['src/**/*.svelte.test.js', 'src/stores/{search,theme}.test.js'],
          setupFiles: ['vitest-browser-svelte', './src/test/msw/setup-browser.js'],
          browser: {
            enabled: true,
            headless: true,
            provider: playwright(),
            instances: [{ browser: 'chromium' }],
          },
        },
      },
    ],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov', 'html'],
      include: ['src/**/*.{js,svelte}'],
      exclude: ['src/test/**', 'src/main.js'],
    },
  },
});
