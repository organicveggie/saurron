<script>
  import { onMount } from 'svelte';
  import { getHistory } from '../lib/api.js';
  import { formatRelative, formatAbs, formatDuration, getDailyAggregates } from '../lib/time.js';
  import StatCard from '../lib/StatCard.svelte';
  import Sparkline from '../lib/Sparkline.svelte';
  import OutcomeChip from '../lib/atoms/OutcomeChip.svelte';

  let cycles = [];
  let loading = true;

  onMount(async () => {
    try {
      const data = await getHistory(1, 100);
      cycles = data.cycles ?? [];
    } catch {
      cycles = [];
    } finally {
      loading = false;
    }
  });

  $: noData = !loading && cycles.length === 0;

  $: recentCycles = cycles.filter(
    (c) => new Date(c.started_at).getTime() >= Date.now() - 7 * 24 * 60 * 60 * 1000,
  );

  $: lastCycle = cycles[0] ?? null;
  $: lastCycleDuration = lastCycle
    ? Math.round((new Date(lastCycle.completed_at) - new Date(lastCycle.started_at)) / 1000)
    : 0;
  $: lastCycleOutcome = lastCycle
    ? lastCycle.failed > 0
      ? 'failed'
      : lastCycle.rolled_back > 0
        ? 'rolled_back'
        : lastCycle.updated > 0
          ? 'updated'
          : 'up_to_date'
    : 'up_to_date';
  $: lastCycleValue = lastCycle ? formatRelative(lastCycle.started_at) : '—';
  $: lastCycleSub = lastCycle
    ? `${formatAbs(lastCycle.started_at)} · ${formatDuration(lastCycleDuration)}`
    : '—';

  $: updatesThisWeek = recentCycles.reduce((a, c) => a + Number(c.updated), 0);
  $: failuresThisWeek = recentCycles.reduce(
    (a, c) => a + Number(c.failed) + Number(c.rolled_back),
    0,
  );
  $: cyclesThisWeek = recentCycles.length;

  $: updatesValue = noData ? '—' : String(updatesThisWeek);
  $: updatesSub = noData ? '—' : `across ${cyclesThisWeek} cycle${cyclesThisWeek === 1 ? '' : 's'}`;

  $: failuresValue = noData ? '—' : String(failuresThisWeek);
  $: failuresSub = noData ? '—' : failuresThisWeek === 0 ? 'all green' : 'review failed cycles';
  $: failuresTone = !noData && failuresThisWeek > 0 ? 'error' : 'neutral';
  $: failuresIcon = !noData && failuresThisWeek > 0 ? 'error' : 'check_circle';

  $: dailyData = getDailyAggregates(cycles);
</script>

<div class="dashboard-content">
  <div class="stat-grid">
    <div class="stat-card-last">
      <StatCard label="Last cycle" value={lastCycleValue} sub={lastCycleSub} icon="schedule">
        {#snippet badge()}
          {#if lastCycle}
            <OutcomeChip outcome={lastCycleOutcome} />
          {/if}
        {/snippet}
      </StatCard>
    </div>

    <StatCard label="Updates this week" value={updatesValue} sub={updatesSub} icon="upgrade">
      {#snippet chart()}
        <Sparkline data={dailyData} />
      {/snippet}
    </StatCard>

    <StatCard
      label="Failures this week"
      value={failuresValue}
      sub={failuresSub}
      tone={failuresTone}
      icon={failuresIcon}
    />
  </div>
</div>

<style>
  .dashboard-content {
    padding: 24px 32px 32px;
  }

  .stat-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 16px;
    margin-bottom: 24px;
  }

  @media (max-width: 899px) {
    .dashboard-content {
      padding: 18px 20px 24px;
    }

    .stat-grid {
      grid-template-columns: repeat(2, 1fr);
      gap: 12px;
      margin-bottom: 18px;
    }

    .stat-card-last {
      grid-column: 1 / -1;
    }
  }
</style>
