import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';

function jsonResponse(body) {
  return { ok: true, json: () => Promise.resolve(body) };
}

async function loadHealthStore() {
  const mod = await import('./health.js');
  return mod.health;
}

describe('health store', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('populates the store from a successful /v1/health response', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        jsonResponse({
          updating: true,
          version: 'v1.2.3',
          hostname: 'saurron-host',
          schedule_mode: 'interval',
          schedule_interval_secs: 3600,
          schedule_cron: null,
          next_run_at: '2026-06-08T12:00:00Z',
          cycle_progress: { phase: 'pulling', pct: 40 },
        }),
      ),
    );

    const health = await loadHealthStore();
    await vi.advanceTimersByTimeAsync(0);

    expect(get(health)).toEqual({
      updating: true,
      version: 'v1.2.3',
      hostname: 'saurron-host',
      schedule_mode: 'interval',
      schedule_interval_secs: 3600,
      schedule_cron: null,
      next_run_at: '2026-06-08T12:00:00Z',
      cycle_progress: { phase: 'pulling', pct: 40 },
    });
  });

  it('falls back to defaults for missing response fields', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse({})));

    const health = await loadHealthStore();
    await vi.advanceTimersByTimeAsync(0);

    expect(get(health)).toEqual({
      updating: false,
      version: '',
      hostname: '',
      schedule_mode: '',
      schedule_interval_secs: null,
      schedule_cron: null,
      next_run_at: null,
      cycle_progress: null,
    });
  });

  it('keeps the last-known value when the fetch rejects', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('network error')));

    const health = await loadHealthStore();
    const initial = get(health);
    await vi.advanceTimersByTimeAsync(0);

    expect(get(health)).toEqual(initial);
  });

  it('refetches on the 5s interval', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse({ version: 'v1' }))
      .mockResolvedValueOnce(jsonResponse({ version: 'v2' }));
    vi.stubGlobal('fetch', fetchMock);

    const health = await loadHealthStore();
    await vi.advanceTimersByTimeAsync(0);
    expect(get(health).version).toBe('v1');

    await vi.advanceTimersByTimeAsync(5000);
    expect(get(health).version).toBe('v2');
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });
});
