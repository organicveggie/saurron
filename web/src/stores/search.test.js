import { afterEach, describe, expect, it } from 'vitest';
import { get } from 'svelte/store';
import { searchQuery } from './search.js';

describe('searchQuery store', () => {
  afterEach(() => {
    searchQuery.set('');
  });

  it('starts empty', () => {
    expect(get(searchQuery)).toBe('');
  });

  it('is a writable store', () => {
    searchQuery.set('nginx');

    expect(get(searchQuery)).toBe('nginx');
  });
});
