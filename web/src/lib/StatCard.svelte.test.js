import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import StatCard from './StatCard.svelte';
import StatCardWithSnippets from '../test/wrappers/StatCardWithSnippets.svelte';

describe('StatCard', () => {
  it('renders label, value, and sub', async () => {
    const { container } = render(StatCard, { label: 'Cycles', value: '12', sub: 'last 7 days' });
    await expect.element(container.querySelector('.type-overline')).toHaveTextContent('Cycles');
    await expect.element(container.querySelector('.type-display')).toHaveTextContent('12');
    await expect.element(container.querySelector('.type-body-sm')).toHaveTextContent('last 7 days');
  });

  it('uses neutral background by default', () => {
    const { container } = render(StatCard, { label: 'L', value: '0', sub: '' });
    expect(container.querySelector('.stat-card').getAttribute('style')).toContain('var(--sc-low)');
  });

  it('uses error-container background when tone is error', () => {
    const { container } = render(StatCard, { label: 'L', value: '0', sub: '', tone: 'error' });
    expect(container.querySelector('.stat-card').getAttribute('style')).toContain(
      'var(--error-container)',
    );
  });

  it('renders icon span when icon prop is provided', async () => {
    const { container } = render(StatCard, { label: 'L', value: '0', sub: '', icon: 'check' });
    const iconEl = container.querySelector('.stat-card .ms');
    expect(iconEl).not.toBeNull();
    await expect.element(iconEl).toHaveTextContent('check');
  });

  it('omits icon span when icon prop is absent', () => {
    const { container } = render(StatCard, { label: 'L', value: '0', sub: '' });
    expect(container.querySelector('.stat-card .ms')).toBeNull();
  });

  it('renders badge snippet when provided', () => {
    const { container } = render(StatCardWithSnippets, { label: 'L', value: '0', sub: '' });
    expect(container.querySelector('.test-badge')).not.toBeNull();
  });

  it('renders chart snippet inside chart-wrap when provided', () => {
    const { container } = render(StatCardWithSnippets, { label: 'L', value: '0', sub: '' });
    expect(container.querySelector('.chart-wrap .test-chart')).not.toBeNull();
  });

  it('omits chart-wrap when chart snippet is absent', () => {
    const { container } = render(StatCard, { label: 'L', value: '0', sub: '' });
    expect(container.querySelector('.chart-wrap')).toBeNull();
  });
});
