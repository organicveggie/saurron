<script>
  let { running = false } = $props();
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
    <div class="type-mono type-num countdown">—</div>
    <div class="type-body-sm" style="color: var(--on-surface-muted)">— · — watched</div>
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
