import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import ContainerOutcomeRow from './ContainerOutcomeRow.svelte';

const withImages = {
  name: 'web',
  outcome: 'updated',
  old_image: 'ghcr.io/example/web:1.3.0',
  new_image: 'ghcr.io/example/web:1.4.0',
};

const failedNoNew = {
  name: 'web',
  outcome: 'failed',
  old_image: 'ghcr.io/example/web:1.3.0',
  reason: 'health check timed out',
};

const noReason = {
  name: 'cache',
  outcome: 'failed',
  old_image: 'redis:7',
};

describe('ContainerOutcomeRow', () => {
  it('renders container name', async () => {
    const screen = render(ContainerOutcomeRow, { container: withImages });
    await expect.element(screen.container.querySelector('.name')).toHaveTextContent('web');
  });

  it('renders old image tag', async () => {
    const screen = render(ContainerOutcomeRow, { container: withImages });
    await expect
      .element(screen.container.querySelectorAll('.image-ref')[0])
      .toHaveTextContent('1.3.0');
  });

  it('renders new image tag when new_image is present', async () => {
    const screen = render(ContainerOutcomeRow, { container: withImages });
    await expect
      .element(screen.container.querySelectorAll('.image-ref')[1])
      .toHaveTextContent('1.4.0');
  });

  it('renders arrow separator', async () => {
    const screen = render(ContainerOutcomeRow, { container: withImages });
    await expect
      .element(screen.container.querySelector('.arrow-icon'))
      .toHaveTextContent('arrow_forward');
  });

  it('renders reason when new_image is absent', async () => {
    const screen = render(ContainerOutcomeRow, { container: failedNoNew });
    await expect
      .element(screen.container.querySelectorAll('.image-ref')[1])
      .toHaveTextContent('health check timed out');
  });

  it('renders em-dash when new_image and reason are both absent', async () => {
    const screen = render(ContainerOutcomeRow, { container: noReason });
    await expect.element(screen.container.querySelectorAll('.image-ref')[1]).toHaveTextContent('—');
  });

  it.each([
    ['failed', 'var(--error)'],
    ['rolled_back', 'var(--warning)'],
    ['updated', 'var(--success)'],
    ['skipped', 'var(--neutral)'],
    ['up_to_date', 'var(--neutral)'],
  ])('dot background is %s color for outcome %s', (outcome, expectedColor) => {
    const screen = render(ContainerOutcomeRow, { container: { ...withImages, outcome } });
    expect(screen.container.querySelector('.dot').getAttribute('style')).toContain(expectedColor);
  });

  it('dot background falls back to neutral for unknown outcome', () => {
    const screen = render(ContainerOutcomeRow, {
      container: { ...withImages, outcome: 'mystery' },
    });
    expect(screen.container.querySelector('.dot').getAttribute('style')).toContain(
      'var(--neutral)',
    );
  });
});
