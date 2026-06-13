import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';
import FilterDropdown from './FilterDropdown.svelte';

const options = ['All', 'Updates', 'Failures'];

describe('FilterDropdown', () => {
  it('renders the trigger button with icon and current value', async () => {
    const { container } = render(FilterDropdown, { icon: 'filter_list', options, value: 'All' });
    const btn = container.querySelector('.filter-btn');
    await expect.element(btn).toHaveTextContent('All');
    await expect.element(btn.querySelector('.icon')).toHaveTextContent('filter_list');
  });

  it('opens dropdown on button click', async () => {
    const { container } = render(FilterDropdown, { icon: 'filter_list', options, value: 'All' });
    expect(container.querySelector('.dropdown')).toBeNull();
    await userEvent.click(container.querySelector('.filter-btn'));
    expect(container.querySelector('.dropdown')).not.toBeNull();
  });

  it('closes dropdown on second button click', async () => {
    const { container } = render(FilterDropdown, { icon: 'filter_list', options, value: 'All' });
    const btn = container.querySelector('.filter-btn');
    await userEvent.click(btn);
    await userEvent.click(btn);
    expect(container.querySelector('.dropdown')).toBeNull();
  });

  it('selects an option, updates displayed value, and closes dropdown', async () => {
    const { container } = render(FilterDropdown, { icon: 'filter_list', options, value: 'All' });
    const btn = container.querySelector('.filter-btn');
    await userEvent.click(btn);
    await userEvent.click(container.querySelectorAll('.opt')[1]); // 'Updates'
    expect(container.querySelector('.dropdown')).toBeNull();
    await expect.element(btn).toHaveTextContent('Updates');
  });

  it('closes dropdown on outside mousedown', async () => {
    const { container } = render(FilterDropdown, { icon: 'filter_list', options, value: 'All' });
    await userEvent.click(container.querySelector('.filter-btn'));
    expect(container.querySelector('.dropdown')).not.toBeNull();
    await userEvent.click(document.body);
    expect(container.querySelector('.dropdown')).toBeNull();
  });

  it('shows colored check icon for the active option and transparent for others', async () => {
    const { container } = render(FilterDropdown, {
      icon: 'filter_list',
      options,
      value: 'Updates',
    });
    await userEvent.click(container.querySelector('.filter-btn'));
    const opts = container.querySelectorAll('.opt');
    expect(opts[1].querySelector('.check-icon').getAttribute('style')).toContain('var(--primary)');
    expect(opts[0].querySelector('.check-icon').getAttribute('style')).toContain('transparent');
  });
});
