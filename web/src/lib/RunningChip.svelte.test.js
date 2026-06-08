import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import RunningChip from './RunningChip.svelte';

describe('RunningChip', () => {
  it('renders the running state', async () => {
    const screen = render(RunningChip, { running: true });
    const chip = screen.container.querySelector('.running-chip');

    await expect.element(chip).toHaveClass('live');
    await expect.element(chip).toHaveTextContent('running');
  });

  it('renders the idle state', async () => {
    const screen = render(RunningChip, { running: false });
    const chip = screen.container.querySelector('.running-chip');

    await expect.element(chip).toHaveClass('idle');
    await expect.element(chip).toHaveTextContent('idle');
  });

  it('defaults to idle when running is omitted', async () => {
    const screen = render(RunningChip);
    const chip = screen.container.querySelector('.running-chip');

    await expect.element(chip).toHaveClass('idle');
    await expect.element(chip).toHaveTextContent('idle');
  });
});
