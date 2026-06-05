<script>
  let { data = [] } = $props();

  const W = 120;
  const H = 36;
  const PAD = 3;

  const maxVal = $derived(Math.max(1, ...data.map((d) => d.updated)));

  const points = $derived(
    data.map((d, i) => ({
      x: data.length > 1 ? (i / (data.length - 1)) * W : W / 2,
      y: H - PAD - (d.updated / maxVal) * (H - PAD * 2),
      bad: d.failed > 0 || d.rolled_back > 0,
    })),
  );

  const polyline = $derived(points.map((p) => `${p.x},${p.y}`).join(' '));

  const area = $derived(
    points.length > 1
      ? `M ${points[0].x},${H} L ${points[0].x},${points[0].y} ` +
          points
            .slice(1)
            .map((p) => `L ${p.x},${p.y}`)
            .join(' ') +
          ` L ${points[points.length - 1].x},${H} Z`
      : '',
  );
</script>

{#if points.length > 0}
  <svg
    width="100%"
    viewBox="0 0 {W} {H}"
    preserveAspectRatio="none"
    class="sparkline"
    aria-hidden="true"
  >
    {#if area}
      <path d={area} class="area" />
    {/if}
    {#if polyline}
      <polyline points={polyline} class="line" />
    {/if}
    {#each points as p, i (i)}
      {#if p.bad}
        <circle cx={p.x} cy={p.y} r="2.5" class="bad-dot" />
      {/if}
    {/each}
  </svg>
{/if}

<style>
  .sparkline {
    display: block;
    overflow: visible;
  }

  .area {
    fill: color-mix(in oklch, var(--primary) 18%, transparent);
  }

  .line {
    fill: none;
    stroke: var(--primary);
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .bad-dot {
    fill: var(--error);
  }
</style>
