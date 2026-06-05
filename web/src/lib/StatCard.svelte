<script>
  let { label, value, sub, tone = 'neutral', icon, badge, chart } = $props();
</script>

<div
  class="card stat-card"
  style="
    background: {tone === 'error' ? 'var(--error-container)' : 'var(--sc-low)'};
    color: {tone === 'error' ? 'var(--on-error-container)' : 'var(--on-surface)'};
  "
>
  <div class="header">
    {#if icon}
      <span class="ms" style="font-size: 18px; color: inherit; opacity: 0.85">{icon}</span>
    {/if}
    <span class="type-overline" style="color: inherit; opacity: 0.8">{label}</span>
    <span class="spacer"></span>
    {#if badge}
      {@render badge()}
    {/if}
  </div>
  <div class="value-row">
    <div
      class="type-display type-num"
      style="font-size: 40px; font-weight: 500; letter-spacing: -0.02em; color: inherit"
    >
      {value}
    </div>
    {#if chart}
      <div class="chart-wrap">
        {@render chart()}
      </div>
    {/if}
  </div>
  <div
    class="type-body-sm"
    style="color: {tone === 'error' ? 'inherit' : 'var(--on-surface-muted)'}; opacity: {tone ===
    'error'
      ? 0.85
      : 1}"
  >
    {sub}
  </div>
</div>

<style>
  .stat-card {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 132px;
  }

  .header {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .spacer {
    flex: 1;
  }

  .value-row {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 12px;
  }

  .chart-wrap {
    width: 120px;
    flex-shrink: 0;
  }

  @media (max-width: 899px) {
    .chart-wrap {
      width: 88px;
    }
  }
</style>
