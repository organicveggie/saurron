import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  formatAbs,
  formatDuration,
  formatRelative,
  formatTime,
  getDailyAggregates,
  groupByDay,
} from './time.js';

const NOW = new Date('2026-06-08T12:00:00.000Z');

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW);
});

afterEach(() => {
  vi.useRealTimers();
});

function isoMinutesAgo(mins) {
  return new Date(NOW.getTime() - mins * 60_000).toISOString();
}

function isoHoursAgo(hrs) {
  return isoMinutesAgo(hrs * 60);
}

function isoDaysAgo(days) {
  return isoHoursAgo(days * 24);
}

describe('formatRelative', () => {
  it('renders "just now" for timestamps under a minute old', () => {
    expect(formatRelative(isoMinutesAgo(0))).toBe('just now');
  });

  it('renders minutes for timestamps under an hour old', () => {
    expect(formatRelative(isoMinutesAgo(1))).toBe('1m ago');
    expect(formatRelative(isoMinutesAgo(59))).toBe('59m ago');
  });

  it('renders hours once a full hour has passed', () => {
    expect(formatRelative(isoMinutesAgo(60))).toBe('1h ago');
    expect(formatRelative(isoHoursAgo(23))).toBe('23h ago');
  });

  it('renders days once a full day has passed', () => {
    expect(formatRelative(isoHoursAgo(24))).toBe('1d ago');
    expect(formatRelative(isoDaysAgo(5))).toBe('5d ago');
  });
});

describe('formatAbs', () => {
  it('formats month, day, hour and minute', () => {
    const iso = '2026-06-08T09:05:00.000Z';

    expect(formatAbs(iso)).toBe(
      new Date(iso).toLocaleString(undefined, {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
      }),
    );
  });
});

describe('formatTime', () => {
  it('formats as 24-hour HH:MM', () => {
    const iso = '2026-06-08T09:05:00.000Z';

    expect(formatTime(iso)).toBe(
      new Date(iso).toLocaleTimeString(undefined, {
        hour: '2-digit',
        minute: '2-digit',
        hour12: false,
      }),
    );
  });
});

describe('formatDuration', () => {
  it('renders sub-minute durations in seconds', () => {
    expect(formatDuration(45)).toBe('45s');
  });

  it('renders exact-minute durations without seconds', () => {
    expect(formatDuration(120)).toBe('2m');
  });

  it('renders minute-plus-seconds durations', () => {
    expect(formatDuration(125)).toBe('2m 5s');
  });
});

describe('groupByDay', () => {
  function cycle(id, isoStartedAt) {
    return { id, started_at: isoStartedAt };
  }

  it('labels same-day entries as Today and groups them together', () => {
    const groups = groupByDay([
      cycle(1, '2026-06-08T08:00:00.000Z'),
      cycle(2, '2026-06-08T10:00:00.000Z'),
    ]);

    expect(groups).toHaveLength(1);
    expect(groups[0].label).toBe('Today');
    expect(groups[0].cycles.map((c) => c.id)).toEqual([1, 2]);
  });

  it('labels the previous day as Yesterday', () => {
    const groups = groupByDay([cycle(1, isoDaysAgo(1))]);

    expect(groups[0].label).toBe('Yesterday');
  });

  it('labels 2-6 days ago with the weekday name', () => {
    const startedAt = isoDaysAgo(3);
    const groups = groupByDay([cycle(1, startedAt)]);

    expect(groups[0].label).toBe(
      new Date(startedAt).toLocaleDateString(undefined, { weekday: 'long' }),
    );
  });

  it('labels 7+ days ago with the full short date', () => {
    const startedAt = isoDaysAgo(10);
    const groups = groupByDay([cycle(1, startedAt)]);

    expect(groups[0].label).toBe(
      new Date(startedAt).toLocaleDateString(undefined, {
        weekday: 'short',
        month: 'short',
        day: 'numeric',
      }),
    );
  });

  it('preserves first-seen ordering of distinct day groups', () => {
    const groups = groupByDay([
      cycle(1, isoDaysAgo(1)),
      cycle(2, isoDaysAgo(0)),
      cycle(3, isoDaysAgo(1)),
    ]);

    expect(groups.map((g) => g.label)).toEqual(['Yesterday', 'Today']);
    expect(groups[0].cycles.map((c) => c.id)).toEqual([1, 3]);
    expect(groups[1].cycles.map((c) => c.id)).toEqual([2]);
  });
});

describe('getDailyAggregates', () => {
  it('returns 7 buckets summing same-day cycles, oldest first', () => {
    const todayMatch = { started_at: NOW.toISOString(), updated: 2, failed: 1, rolled_back: 0 };
    const todayOther = {
      started_at: new Date(NOW.getTime() - 60_000).toISOString(),
      updated: 1,
      failed: 0,
      rolled_back: 1,
    };
    const sixDaysAgo = { started_at: isoDaysAgo(6), updated: 3, failed: 0, rolled_back: 0 };

    const buckets = getDailyAggregates([todayMatch, todayOther, sixDaysAgo]);

    expect(buckets).toHaveLength(7);
    expect(buckets[0]).toEqual({ updated: 3, failed: 0, rolled_back: 0 });
    expect(buckets[6]).toEqual({ updated: 3, failed: 1, rolled_back: 1 });
  });

  it('returns all-zero buckets for empty input', () => {
    const buckets = getDailyAggregates([]);

    expect(buckets).toHaveLength(7);
    for (const bucket of buckets) {
      expect(bucket).toEqual({ updated: 0, failed: 0, rolled_back: 0 });
    }
  });
});
