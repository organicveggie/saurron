import { afterEach, describe, expect, it } from 'vitest';
import { get } from 'svelte/store';
import { theme } from './theme.js';

describe('theme store', () => {
  const original = get(theme);

  afterEach(() => {
    theme.set(original);
  });

  it('initializes from the document theme attribute', () => {
    expect(get(theme)).toBe(document.documentElement.dataset.theme ?? 'light');
  });

  it('writes the data-theme attribute and localStorage on change', () => {
    const next = original === 'dark' ? 'light' : 'dark';

    theme.set(next);

    expect(document.documentElement.dataset.theme).toBe(next);
    expect(localStorage.getItem('theme')).toBe(next);
  });
});
