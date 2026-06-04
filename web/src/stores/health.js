import { writable } from 'svelte/store';

export const health = writable({ updating: false, version: '', hostname: '' });

async function poll() {
  try {
    const r = await fetch('/v1/health');
    if (r.ok) {
      const d = await r.json();
      health.set({
        updating: d.updating ?? false,
        version: d.version ?? '',
        hostname: d.hostname ?? '',
      });
    }
  } catch {
    // swallow network errors; store keeps last known value
  }
}

poll();
setInterval(poll, 5000);
