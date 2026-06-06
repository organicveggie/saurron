import { writable } from 'svelte/store';

export const health = writable({
  updating: false,
  version: '',
  hostname: '',
  schedule_mode: '',
  schedule_interval_secs: null,
  schedule_cron: null,
  next_run_at: null,
});

async function poll() {
  try {
    const r = await fetch('/v1/health');
    if (r.ok) {
      const d = await r.json();
      health.set({
        updating: d.updating ?? false,
        version: d.version ?? '',
        hostname: d.hostname ?? '',
        schedule_mode: d.schedule_mode ?? '',
        schedule_interval_secs: d.schedule_interval_secs ?? null,
        schedule_cron: d.schedule_cron ?? null,
        next_run_at: d.next_run_at ?? null,
      });
    }
  } catch {
    // swallow network errors; store keeps last known value
  }
}

poll();
setInterval(poll, 5000);
