<script>
  import { onMount } from 'svelte';
  import { getHistory } from '../lib/api.js';
  import {
    formatRelative,
    formatAbs,
    formatDuration,
    getDailyAggregates,
    groupByDay,
  } from '../lib/time.js';
  import { searchQuery } from '../stores/search.js';
  import StatCard from '../lib/StatCard.svelte';
  import Sparkline from '../lib/Sparkline.svelte';
  import OutcomeChip from '../lib/atoms/OutcomeChip.svelte';
  import FilterDropdown from '../lib/FilterDropdown.svelte';
  import TimelineEvent from '../lib/TimelineEvent.svelte';

  const PER_PAGE = 20;

  const TIME_OPTIONS = ['Last 24 hours', 'Last 7 days', 'Last 30 days', 'Last 90 days', 'All time'];
  const TRIGGER_OPTIONS = ['All triggers', 'Scheduled', 'Manual', 'HTTP API'];
  const SEGMENTS = ['All', 'Updates', 'Failures'];

  const TRIGGER_MAP = {
    'All triggers': null,
    Scheduled: 'scheduled',
    Manual: 'manual',
    'HTTP API': 'http_api',
  };

  const TIME_MS = {
    'Last 24 hours': 24 * 3600 * 1000,
    'Last 7 days': 7 * 86400 * 1000,
    'Last 30 days': 30 * 86400 * 1000,
    'Last 90 days': 90 * 86400 * 1000,
  };

  let cycles = $state([]);
  let total = $state(0);
  let currentPage = $state(1);
  let loading = $state(true);
  let loadingMore = $state(false);
  let expandedId = $state(null);

  let timeWindow = $state('Last 7 days');
  let triggerFilter = $state('All triggers');
  let segment = $state('All');

  onMount(async () => {
    try {
      const data = await getHistory(1, PER_PAGE);
      cycles = data.cycles ?? [];
      total = data.total ?? 0;
    } catch {
      cycles = [];
      total = 0;
    } finally {
      loading = false;
    }
  });

  async function loadOlder() {
    loadingMore = true;
    try {
      const data = await getHistory(currentPage + 1, PER_PAGE);
      cycles = [...cycles, ...(data.cycles ?? [])];
      total = data.total ?? total;
      currentPage++;
    } finally {
      loadingMore = false;
    }
  }

  function toggleExpand(id) {
    expandedId = expandedId === id ? null : id;
  }

  // --- Stat card derivations (over all loaded cycles) ---

  let noData = $derived(!loading && cycles.length === 0);

  let recentCycles = $derived(
    cycles.filter((c) => new Date(c.started_at).getTime() >= Date.now() - 7 * 24 * 60 * 60 * 1000),
  );

  let lastCycle = $derived(cycles[0] ?? null);
  let lastCycleDuration = $derived(
    lastCycle?.completed_at
      ? Math.max(
          0,
          Math.round((new Date(lastCycle.completed_at) - new Date(lastCycle.started_at)) / 1000),
        )
      : 0,
  );
  let lastCycleOutcome = $derived(
    lastCycle
      ? lastCycle.failed > 0
        ? 'failed'
        : lastCycle.rolled_back > 0
          ? 'rolled_back'
          : lastCycle.updated > 0
            ? 'updated'
            : 'up_to_date'
      : 'up_to_date',
  );
  let lastCycleValue = $derived(lastCycle ? formatRelative(lastCycle.started_at) : '—');
  let lastCycleSub = $derived(
    lastCycle ? `${formatAbs(lastCycle.started_at)} · ${formatDuration(lastCycleDuration)}` : '—',
  );

  let updatesThisWeek = $derived(recentCycles.reduce((a, c) => a + Number(c.updated), 0));
  let failuresThisWeek = $derived(
    recentCycles.reduce((a, c) => a + Number(c.failed) + Number(c.rolled_back), 0),
  );
  let cyclesThisWeek = $derived(recentCycles.length);

  let updatesValue = $derived(noData ? '—' : String(updatesThisWeek));
  let updatesSub = $derived(
    noData ? '—' : `across ${cyclesThisWeek} cycle${cyclesThisWeek === 1 ? '' : 's'}`,
  );

  let failuresValue = $derived(noData ? '—' : String(failuresThisWeek));
  let failuresSub = $derived(
    noData ? '—' : failuresThisWeek === 0 ? 'all green' : 'review failed cycles',
  );
  let failuresTone = $derived(!noData && failuresThisWeek > 0 ? 'error' : 'neutral');
  let failuresIcon = $derived(!noData && failuresThisWeek > 0 ? 'error' : 'check_circle');

  let dailyData = $derived(getDailyAggregates(cycles));

  // --- Filter pipeline ---

  let afterTimeWindow = $derived(
    timeWindow === 'All time'
      ? cycles
      : cycles.filter(
          (c) => new Date(c.started_at).getTime() >= Date.now() - (TIME_MS[timeWindow] ?? 0),
        ),
  );

  let afterTrigger = $derived(
    TRIGGER_MAP[triggerFilter]
      ? afterTimeWindow.filter((c) => c.trigger === TRIGGER_MAP[triggerFilter])
      : afterTimeWindow,
  );

  let afterSegment = $derived(
    segment === 'Updates'
      ? afterTrigger.filter((c) => c.updated > 0 || c.failed > 0 || c.rolled_back > 0)
      : segment === 'Failures'
        ? afterTrigger.filter((c) => c.failed > 0 || c.rolled_back > 0)
        : afterTrigger,
  );

  let filteredCycles = $derived(
    $searchQuery.trim()
      ? afterSegment.filter((c) =>
          (c.containers ?? []).some((cc) =>
            cc.name.toLowerCase().includes($searchQuery.toLowerCase()),
          ),
        )
      : afterSegment,
  );

  let hasMore = $derived(cycles.length < total);

  let dayGroups = $derived(groupByDay(filteredCycles));

  let filtersActive = $derived(
    timeWindow !== 'Last 7 days' ||
      triggerFilter !== 'All triggers' ||
      segment !== 'All' ||
      $searchQuery.trim() !== '',
  );
</script>

<div class="dashboard-content">
  <!-- Stat cards -->
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

  <!-- Timeline header -->
  <div class="timeline-header">
    <div class="type-h2">Recent</div>
    <span class="chip outlined">{filteredCycles.length} shown</span>
    <div class="spacer"></div>

    <div class="segment-toggle">
      {#each SEGMENTS as seg (seg)}
        <button class="seg-btn" class:active={segment === seg} onclick={() => (segment = seg)}>
          {seg}
        </button>
      {/each}
    </div>

    <FilterDropdown bind:value={timeWindow} icon="event" options={TIME_OPTIONS} />
    <FilterDropdown bind:value={triggerFilter} icon="bolt" options={TRIGGER_OPTIONS} />
  </div>

  <!-- Timeline rail -->
  <div class="timeline-rail">
    {#if loading}
      <div class="loading-state type-body-sm">Loading…</div>
    {:else if dayGroups.length === 0}
      <!-- Empty state -->
      <div class="empty-state">
        <span class="ms empty-icon">{filtersActive ? 'filter_list_off' : 'history'}</span>
        <div class="type-title">
          {filtersActive ? 'No cycles match your filters' : 'No update cycles yet'}
        </div>
        <div class="type-body-sm empty-sub">
          {filtersActive
            ? 'Try adjusting the time window, trigger filter, or search query.'
            : 'Saurron will record each update cycle here once it starts running.'}
        </div>
      </div>
    {:else}
      {#each dayGroups as group, gi (group.label)}
        {@const groupUpdated = group.cycles.reduce((a, c) => a + Number(c.updated), 0)}
        {@const groupIncidents = group.cycles.reduce(
          (a, c) => a + Number(c.failed) + Number(c.rolled_back),
          0,
        )}
        <div class="day-group" class:last={gi === dayGroups.length - 1}>
          <!-- Day header -->
          <div class="day-header">
            <div class="day-icon">
              <span class="ms" style="font-size: 14px">today</span>
            </div>
            <div class="type-title day-label">{group.label}</div>
            <div class="type-body-sm day-meta">
              {group.cycles.length}
              {group.cycles.length === 1 ? 'cycle' : 'cycles'} · {groupUpdated} updated{groupIncidents >
              0
                ? `, ${groupIncidents} incident${groupIncidents > 1 ? 's' : ''}`
                : ''}
            </div>
            <div class="day-rule"></div>
          </div>

          <!-- Events -->
          <div class="events-wrap">
            <div class="timeline-rule"></div>
            {#each group.cycles as cycle (cycle.id)}
              <TimelineEvent
                {cycle}
                isExpanded={expandedId === cycle.id}
                onToggle={() => toggleExpand(cycle.id)}
              />
            {/each}
          </div>
        </div>
      {/each}

      {#if hasMore}
        <div class="load-older">
          <button class="btn outlined" onclick={loadOlder} disabled={loadingMore}>
            <span class="ms">expand_more</span>
            {loadingMore ? 'Loading…' : `Load older (${total - cycles.length} remaining)`}
          </button>
        </div>
      {/if}
    {/if}
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

  /* Timeline header */

  .timeline-header {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 16px;
  }

  .spacer {
    flex: 1;
  }

  .segment-toggle {
    display: inline-flex;
    height: 36px;
    background: var(--sc-low);
    border-radius: 999px;
    padding: 3px;
    gap: 2px;
    color: var(--on-surface-variant);
    font-size: 12px;
    font-weight: 500;
  }

  @media (max-width: 899px) {
    .segment-toggle {
      display: none;
    }
  }

  .seg-btn {
    height: 30px;
    padding: 0 14px;
    border-radius: 999px;
    border: 0;
    cursor: pointer;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    font-weight: 500;
  }

  .seg-btn.active {
    background: var(--surface);
    color: var(--on-surface);
    box-shadow: var(--e1);
  }

  /* Timeline rail */

  .timeline-rail {
    position: relative;
    padding-left: 8px;
  }

  .day-group {
    margin-bottom: 8px;
  }

  .day-group.last {
    margin-bottom: 0;
  }

  .day-header {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 12px 0 10px;
  }

  .day-icon {
    width: 24px;
    height: 24px;
    border-radius: 999px;
    background: var(--surface);
    border: 1.5px solid var(--outline-variant);
    display: grid;
    place-items: center;
    flex: 0 0 24px;
    color: var(--on-surface-variant);
  }

  .day-label {
    font-weight: 600;
  }

  .day-meta {
    color: var(--on-surface-muted);
  }

  .day-rule {
    flex: 1;
    height: 1px;
    background: var(--outline-variant);
  }

  .events-wrap {
    position: relative;
  }

  .timeline-rule {
    position: absolute;
    left: 11.25px;
    top: 0;
    bottom: 0;
    width: 1.5px;
    background: var(--outline-variant);
  }

  /* Load older */

  .load-older {
    text-align: center;
    padding: 16px 0;
  }

  /* States */

  .loading-state {
    color: var(--on-surface-muted);
    padding: 24px 0;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 48px 0;
    color: var(--on-surface-variant);
    text-align: center;
  }

  .empty-icon {
    font-size: 48px;
    opacity: 0.4;
  }

  .empty-sub {
    color: var(--on-surface-muted);
    max-width: 340px;
  }
</style>
