<script>
  import { health } from '../stores/health.js';

  let { running = false, watchedCount = 0 } = $props();

  function fmtCountdown(isoStr) {
    if (!isoStr) return '—';
    const secs = Math.round((new Date(isoStr) - Date.now()) / 1000);
    if (secs <= 0) return 'now';
    if (secs < 60) return `in ${secs}s`;
    const mins = Math.round(secs / 60);
    if (mins < 60) return `in ${mins}m`;
    const hours = Math.floor(mins / 60);
    const rem = mins % 60;
    return rem > 0 ? `in ${hours}h ${rem}m` : `in ${hours}h`;
  }

  function scheduleLabel(h) {
    if (h.schedule_mode === 'run_once') return '(run once only)';
    if (h.schedule_mode === 'interval') {
      const s = h.schedule_interval_secs;
      if (!s) return '—';
      if (s % 3600 === 0) return `every ${s / 3600}h`;
      if (s % 60 === 0) return `every ${s / 60}m`;
      return `every ${s}s`;
    }
    if (h.schedule_mode === 'cron') return h.schedule_cron ?? '—';
    return '—';
  }

  let countdown = $derived(fmtCountdown($health.next_run_at));
  let sched = $derived(scheduleLabel($health));
  let sub = $derived(`${sched} · ${watchedCount} watched`);
</script>

{#if running}
  <div class="card outlined cycle-card running">
    <div class="row">
      <span class="running-dot"></span>
      <span class="type-label" style="color: var(--primary)">Cycle running</span>
    </div>
    <div class="running-bar"></div>
  </div>
{:else}
  <div class="card outlined cycle-card">
    <div class="row">
      <span class="ms" style="color: var(--success); font-size: 18px">schedule</span>
      <span class="type-label">Next cycle</span>
    </div>
    <div class="type-mono type-num countdown">{countdown}</div>
    <div class="type-body-sm" style="color: var(--on-surface-muted)">{sub}</div>
  </div>
{/if}

<style>
  .cycle-card {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .cycle-card.running {
    background: var(--primary-soft);
    box-shadow: inset 0 0 0 1px color-mix(in oklch, var(--primary) 30%, transparent);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .countdown {
    font-size: 18px;
    font-weight: 600;
  }
</style>
