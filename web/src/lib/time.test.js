import { describe, expect, it } from 'vitest';
import { formatRelative } from './time.js';

describe('formatRelative', () => {
  it('renders "just now" for timestamps under a minute old', () => {
    expect(formatRelative(new Date().toISOString())).toBe('just now');
  });
});
