import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import OutcomeChip from './OutcomeChip.svelte';

const META = {
  updated: { cls: 'success', icon: 'check_circle', label: 'updated' },
  rolled_back: { cls: 'warning', icon: 'undo', label: 'rolled back' },
  failed: { cls: 'error', icon: 'error', label: 'failed' },
  skipped: { cls: 'neutral', icon: 'skip_next', label: 'skipped' },
  up_to_date: { cls: 'neutral', icon: 'check', label: 'up-to-date' },
};

describe('OutcomeChip', () => {
  it.each(Object.entries(META))('renders the %s outcome', async (outcome, meta) => {
    const screen = render(OutcomeChip, { outcome });
    const chip = screen.container.querySelector('.chip');

    await expect.element(chip).toHaveClass(meta.cls);
    await expect.element(chip).toHaveTextContent(meta.label);
    await expect.element(chip.querySelector('.ms')).toHaveTextContent(meta.icon);
  });

  it('falls back to up_to_date for an unknown outcome', async () => {
    const screen = render(OutcomeChip, { outcome: 'mystery' });
    const chip = screen.container.querySelector('.chip');

    await expect.element(chip).toHaveClass(META.up_to_date.cls);
    await expect.element(chip).toHaveTextContent(META.up_to_date.label);
  });

  it('applies dense inline sizing when dense is true', () => {
    const { container } = render(OutcomeChip, { outcome: 'updated', dense: true });
    const chip = container.querySelector('.chip');
    const icon = chip.querySelector('.ms');

    expect(chip.getAttribute('style')).toContain('height: 22px');
    expect(icon.getAttribute('style')).toContain('font-size: 13px');
  });

  it('uses default sizing when dense is false', () => {
    const { container } = render(OutcomeChip, { outcome: 'updated' });
    const chip = container.querySelector('.chip');
    const icon = chip.querySelector('.ms');

    expect(chip.getAttribute('style')).toBeFalsy();
    expect(icon.getAttribute('style')).toContain('font-size: 14px');
  });
});
