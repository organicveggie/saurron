<script>
  import StatCard from '../../lib/StatCard.svelte';
  let props = $props();
</script>

<StatCard {...props}>
  {#snippet badge()}
    <span class="test-badge">badge-content</span>
  {/snippet}
  {#snippet chart()}
    <span class="test-chart">chart-content</span>
  {/snippet}
</StatCard>
