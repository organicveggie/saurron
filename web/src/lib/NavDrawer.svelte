<script>
  import { onMount } from 'svelte';
  import { router, push } from 'svelte-spa-router';
  import { health } from '../stores/health.js';
  import { getContainers } from './api.js';
  import RunningChip from './RunningChip.svelte';
  import CycleStatusCard from './CycleStatusCard.svelte';

  let { variant = 'standard' } = $props();

  const PREVIEW_COUNT = 5;
  let containers = $state([]);
  let showAll = $state(false);

  onMount(async () => {
    try {
      containers = await getContainers();
    } catch {
      // swallow API errors; drawer shows empty Watched section
    }
  });

  const NAV_ITEMS = [
    { route: '/', label: 'Dashboard', icon: 'dashboard' },
    { route: '/update', label: 'Manual update', icon: 'play_circle' },
    { route: '/template', label: 'Template', icon: 'description' },
    { route: '/notifications', label: 'Notifications', icon: 'notifications' },
  ];

  let hiddenCount = $derived(Math.max(0, containers.length - PREVIEW_COUNT));
  let visibleContainers = $derived(showAll ? containers : containers.slice(0, PREVIEW_COUNT));

  function stateColor(state) {
    if (state === 'running') return 'var(--success)';
    if (state === 'monitor_only') return 'var(--info)';
    return 'var(--neutral)';
  }
</script>

{#if variant === 'rail'}
  <nav class="rail">
    <div class="rail-logo" style="color: var(--primary)">
      <svg width="28" height="28" viewBox="0 0 40 40" aria-label="Saurron" fill="none">
        <path
          d="M20 6 C 30 6, 36 14, 38 20 C 36 26, 30 34, 20 34 C 10 34, 4 26, 2 20 C 4 14, 10 6, 20 6 Z"
          fill="none"
          stroke="currentColor"
          stroke-width="2.5"
          stroke-linejoin="round"
        />
        <ellipse cx="20" cy="20" rx="4" ry="9" fill="currentColor" />
        <ellipse cx="20" cy="20" rx="1.5" ry="5" fill="var(--surface)" />
      </svg>
      {#if $health.updating}
        <span class="running-dot rail-dot"></span>
      {/if}
    </div>
    <div class="rail-sep"></div>
    {#each NAV_ITEMS as item (item.route)}
      <button
        class="rail-btn"
        class:rail-btn-active={router.location === item.route}
        title={item.label}
        onclick={() => push(item.route)}
      >
        <span class="ms" class:fill={router.location === item.route} style="font-size: 24px"
          >{item.icon}</span
        >
      </button>
    {/each}
    <div class="spacer"></div>
    <div class="cycle-badge" title={$health.updating ? 'Cycle running' : 'Next cycle: unknown'}>
      {#if $health.updating}
        <span class="running-dot" style="width: 10px; height: 10px"></span>
        <span class="type-overline" style="font-size: 9px; color: var(--primary)">RUNNING</span>
      {:else}
        <span class="ms" style="font-size: 18px; color: var(--on-surface-variant)">schedule</span>
        <span
          class="type-mono type-num"
          style="font-size: 10px; font-weight: 600; color: var(--on-surface)">—</span
        >
      {/if}
    </div>
  </nav>
{:else}
  <nav class="standard">
    <div class="scroll-area">
      <div class="header">
        <div style="color: var(--primary)">
          <svg width="28" height="28" viewBox="0 0 40 40" aria-label="Saurron" fill="none">
            <path
              d="M20 6 C 30 6, 36 14, 38 20 C 36 26, 30 34, 20 34 C 10 34, 4 26, 2 20 C 4 14, 10 6, 20 6 Z"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
              stroke-linejoin="round"
            />
            <ellipse cx="20" cy="20" rx="4" ry="9" fill="currentColor" />
            <ellipse cx="20" cy="20" rx="1.5" ry="5" fill="var(--surface)" />
          </svg>
        </div>
        <div class="header-text">
          <div class="header-title-row">
            <span class="type-h2">Saurron</span>
            <RunningChip running={$health.updating} />
          </div>
          <div class="type-mono header-meta">
            {$health.version || '—'} · {$health.hostname || '—'}
          </div>
        </div>
      </div>

      <div class="nav-items">
        {#each NAV_ITEMS as item (item.route)}
          <button
            class="nav-item"
            class:nav-item-active={router.location === item.route}
            onclick={() => push(item.route)}
          >
            <span class="ms" class:fill={router.location === item.route} style="font-size: 22px"
              >{item.icon}</span
            >
            <span class="nav-label">{item.label}</span>
          </button>
        {/each}
      </div>

      <div class="watched">
        <div class="type-overline">Watched</div>
        <div class="watched-list">
          {#each visibleContainers as c (c.name)}
            <div class="watched-item">
              <span class="state-dot" style="background: {stateColor(c.state)}"></span>
              <span class="type-mono watched-name">{c.name}</span>
              {#if c.state === 'monitor_only'}
                <span
                  class="ms"
                  title="monitor-only"
                  style="font-size: 14px; color: var(--on-surface-muted)">visibility</span
                >
              {:else if c.state === 'pinned'}
                <span
                  class="ms"
                  title="digest pinned"
                  style="font-size: 14px; color: var(--on-surface-muted)">lock</span
                >
              {/if}
            </div>
          {/each}
          {#if !showAll && hiddenCount > 0}
            <button class="show-toggle" onclick={() => (showAll = true)}>
              + {hiddenCount} more…
            </button>
          {:else if showAll && containers.length > PREVIEW_COUNT}
            <button class="show-toggle" onclick={() => (showAll = false)}>Show less</button>
          {/if}
        </div>
      </div>
    </div>

    <CycleStatusCard progress={$health.cycle_progress} watchedCount={containers.length} />
  </nav>
{/if}

<style>
  /* ── Rail ── */
  .rail {
    width: 64px;
    flex: 0 0 64px;
    background: var(--surface);
    border-right: 1px solid var(--outline-variant);
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 14px 0 10px;
    gap: 6px;
    overflow-y: auto;
  }

  .rail-logo {
    position: relative;
    margin-bottom: 12px;
  }

  .rail-dot {
    position: absolute;
    top: -2px;
    right: -2px;
    width: 8px;
    height: 8px;
    box-shadow: 0 0 0 2px var(--surface);
  }

  .rail-sep {
    width: 28px;
    height: 1px;
    background: var(--outline-variant);
    margin-bottom: 6px;
  }

  .rail-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 48px;
    height: 48px;
    border-radius: 14px;
    border: none;
    background: transparent;
    color: var(--on-surface-variant);
    cursor: pointer;
    transition:
      background 0.15s,
      color 0.15s;
  }

  .rail-btn:hover {
    background: var(--hover);
    color: var(--on-surface);
  }

  .rail-btn-active {
    background: var(--secondary-container);
    color: var(--on-secondary-container);
  }

  .cycle-badge {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 6px 6px 4px;
    border-radius: 12px;
  }

  /* ── Standard ── */
  .standard {
    width: 264px;
    flex: 0 0 264px;
    background: var(--surface);
    border-right: 1px solid var(--outline-variant);
    display: flex;
    flex-direction: column;
    padding: 14px 12px;
    overflow: hidden;
  }

  .scroll-area {
    flex: 1;
    overflow-y: auto;
  }

  .header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px 18px;
  }

  .header-text {
    flex: 1;
    min-width: 0;
  }

  .header-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .header-meta {
    color: var(--on-surface-muted);
    font-size: 11px;
    margin-top: 4px;
  }

  .nav-items {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 48px;
    padding: 0 16px;
    border-radius: var(--r-full);
    border: none;
    background: transparent;
    color: var(--on-surface-variant);
    cursor: pointer;
    font: inherit;
    font-weight: 500;
    font-size: 14px;
    transition: background 0.12s;
    white-space: nowrap;
  }

  .nav-item:not(.nav-item-active):hover {
    background: var(--hover);
  }

  .nav-item-active {
    background: var(--secondary-container);
    color: var(--on-secondary-container);
    font-weight: 600;
  }

  .nav-label {
    flex: 1;
  }

  .watched {
    margin-top: 22px;
    padding: 0 8px;
  }

  .watched-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 8px;
  }

  .watched-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px;
    border-radius: var(--r-sm);
    color: var(--on-surface-variant);
  }

  .state-dot {
    width: 8px;
    height: 8px;
    border-radius: 999px;
    flex-shrink: 0;
  }

  .watched-name {
    flex: 1;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .show-toggle {
    background: transparent;
    border: none;
    cursor: pointer;
    color: var(--primary);
    text-align: left;
    padding: 6px 8px;
    font: inherit;
    font-size: 12px;
    font-weight: 600;
  }

  .spacer {
    flex: 1;
  }
</style>
