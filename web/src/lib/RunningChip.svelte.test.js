import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import RunningChip from './RunningChip.svelte';

describe('RunningChip', () => {
  it('renders the running label', async () => {
    const screen = render(RunningChip, { running: true });

    await expect.element(screen.getByText('running')).toBeVisible();
  });
});
