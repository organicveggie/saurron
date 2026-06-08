import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getContainers, getHealth, getHistory, getHistoryById } from './api.js';

function jsonResponse(body, { ok = true, status = 200 } = {}) {
  return { ok, status, json: () => Promise.resolve(body) };
}

describe('api', () => {
  beforeEach(() => {
    vi.stubGlobal('fetch', vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('getHistory builds the paginated history URL with defaults', async () => {
    fetch.mockResolvedValue(jsonResponse({ cycles: [], total: 0 }));

    await getHistory();

    expect(fetch).toHaveBeenCalledWith('/v1/history?page=1&per_page=20');
  });

  it('getHistory honors explicit page and per_page', async () => {
    fetch.mockResolvedValue(jsonResponse({ cycles: [], total: 0 }));

    await getHistory(3, 10);

    expect(fetch).toHaveBeenCalledWith('/v1/history?page=3&per_page=10');
  });

  it('getHistoryById builds the single-cycle URL', async () => {
    fetch.mockResolvedValue(jsonResponse({ id: 42 }));

    await getHistoryById(42);

    expect(fetch).toHaveBeenCalledWith('/v1/history/42');
  });

  it('getHealth requests the health endpoint', async () => {
    fetch.mockResolvedValue(jsonResponse({ updating: false }));

    await getHealth();

    expect(fetch).toHaveBeenCalledWith('/v1/health');
  });

  it('getContainers requests the containers endpoint', async () => {
    fetch.mockResolvedValue(jsonResponse([]));

    await getContainers();

    expect(fetch).toHaveBeenCalledWith('/v1/containers');
  });

  it('returns the parsed JSON body on success', async () => {
    const body = { cycles: [], total: 0 };
    fetch.mockResolvedValue(jsonResponse(body));

    await expect(getHistory()).resolves.toEqual(body);
  });

  it('throws an HTTP error for non-OK responses', async () => {
    fetch.mockResolvedValue(jsonResponse(null, { ok: false, status: 503 }));

    await expect(getHealth()).rejects.toThrow('HTTP 503');
  });
});
