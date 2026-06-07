<script>
  import { readable } from 'svelte/store';
  import { health } from '../stores/health.js';

  let { progress = null, watchedCount = 0 } = $props();
  let running = $derived(progress !== null);

  const tick = readable(0, (set) => {
    const id = setInterval(() => set(Date.now()), 1000);
    return () => clearInterval(id);
  });

  function fmtElapsed(isoStr) {
    if (!isoStr) return '0:00';
    const ms = Date.now() - new Date(isoStr).getTime();
    if (ms < 0) return '0:00';
    const totalSecs = Math.floor(ms / 1000);
    const mins = Math.floor(totalSecs / 60);
    const secs = totalSecs % 60;
    return `${mins}:${String(secs).padStart(2, '0')}`;
  }

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

  let pct = $derived(
    progress?.total > 0 ? Math.round((progress.scanned / progress.total) * 100) : 0,
  );
  let elapsed = $derived(($tick, fmtElapsed(progress?.started_at)));
  let countdown = $derived(fmtCountdown($health.next_run_at));
  let sched = $derived(scheduleLabel($health));
  let sub = $derived(`${sched} · ${watchedCount} watched`);
</script>

{#if running}
  <div class="card outlined cycle-card running">
    <div class="row">
      <span class="running-dot"></span>
      <span class="type-label" style="color: var(--primary)">Cycle running</span>
      <div style="flex:1"></div>
      <span class="type-mono elapsed">{elapsed}</span>
    </div>
    <div class="running-bar"></div>
    <div class="row justify-between">
      <span class="type-mono current">{progress.current}</span>
      <span class="type-mono count">{progress.scanned}/{progress.total}</span>
    </div>
    <div class="type-body-sm muted">{progress.phase} · {pct}% complete</div>
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

  .justify-between {
    justify-content: space-between;
  }

  .countdown {
    font-size: 18px;
    font-weight: 600;
  }

  .elapsed {
    font-size: 12px;
    color: var(--on-surface-muted);
  }

  .current {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }

  .count {
    font-size: 12px;
    flex-shrink: 0;
  }

  .muted {
    color: var(--on-surface-muted);
    font-size: 11px;
  }
</style>
