import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import TriggerChip from './TriggerChip.svelte';

const META = {
  scheduled: { icon: 'schedule', label: 'scheduled' },
  manual: { icon: 'touch_app', label: 'manual' },
  http_api: { icon: 'webhook', label: 'webhook' },
};

describe('TriggerChip', () => {
  it.each(Object.entries(META))('renders the %s trigger', async (trigger, meta) => {
    const screen = render(TriggerChip, { trigger });
    const chip = screen.container.querySelector('.trigger-chip');

    await expect.element(chip.querySelector('.ms')).toHaveTextContent(meta.icon);
    await expect.element(chip.querySelector('.label')).toHaveTextContent(meta.label);
  });

  it('falls back to scheduled for an unknown trigger', async () => {
    const screen = render(TriggerChip, { trigger: 'mystery' });
    const chip = screen.container.querySelector('.trigger-chip');

    await expect.element(chip.querySelector('.ms')).toHaveTextContent(META.scheduled.icon);
    await expect.element(chip.querySelector('.label')).toHaveTextContent(META.scheduled.label);
  });
});
