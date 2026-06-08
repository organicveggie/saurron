import { describe, expect, it } from 'vitest';
import { imageShortDigest, imageTagOnly } from './image.js';

describe('imageTagOnly', () => {
  it('returns an em-dash placeholder when no ref is given', () => {
    expect(imageTagOnly('')).toBe('—');
    expect(imageTagOnly(null)).toBe('—');
  });

  it('strips a digest suffix and keeps the trailing path segment', () => {
    expect(imageTagOnly('ghcr.io/example/web@sha256:abcdef0123456789')).toBe('web');
  });

  it('keeps the tag and drops the registry/namespace prefix', () => {
    expect(imageTagOnly('ghcr.io/example/web:1.4.0')).toBe('web:1.4.0');
  });

  it('returns the whole ref when there is no path separator', () => {
    expect(imageTagOnly('postgres:16')).toBe('postgres:16');
  });
});

describe('imageShortDigest', () => {
  it('returns an empty string when no ref is given', () => {
    expect(imageShortDigest('')).toBe('');
    expect(imageShortDigest(null)).toBe('');
  });

  it('returns an empty string when the ref has no digest', () => {
    expect(imageShortDigest('ghcr.io/example/web:1.4.0')).toBe('');
  });

  it('slices the first 7 characters of the digest', () => {
    expect(imageShortDigest('postgres:16@sha256:abcdef0123456789abcdef0123456789')).toBe('abcdef0');
  });
});
