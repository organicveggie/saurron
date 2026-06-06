<script>
  import { formatTime, formatAbs, formatDuration } from './time.js';
  import TriggerChip from './atoms/TriggerChip.svelte';
  import ContainerOutcomeRow from './ContainerOutcomeRow.svelte';

  let { cycle, isExpanded = false, onToggle } = $props();

  let duration = $derived(
    cycle.completed_at
      ? Math.max(0, Math.round((new Date(cycle.completed_at) - new Date(cycle.started_at)) / 1000))
      : 0,
  );

  let isQuiet = $derived(!cycle.updated && !cycle.failed && !cycle.rolled_back);

  let activeDotColor = $derived(
    cycle.failed
      ? 'var(--error)'
      : cycle.rolled_back
        ? 'var(--warning)'
        : cycle.updated
          ? 'var(--success)'
          : 'var(--outline)',
  );

  let nonUpToDate = $derived((cycle.containers ?? []).filter((c) => c.outcome !== 'up_to_date'));
  let upToDateCount = $derived(
    (cycle.containers ?? []).filter((c) => c.outcome === 'up_to_date').length,
  );
  let previewContainers = $derived(nonUpToDate.filter((c) => c.outcome !== 'skipped').slice(0, 4));
</script>

<div class="event" class:quiet={isQuiet}>
  <div class="dot" style="background: {isQuiet ? 'var(--outline-variant)' : activeDotColor}"></div>

  <div
    class="card event-card"
    class:expanded={isExpanded}
    class:condensed={isQuiet && !isExpanded}
    role="button"
    tabindex="0"
    onclick={onToggle}
    onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && onToggle()}
  >
    <div class="row">
      <div class="time-col">
        <div class="type-mono type-num time">{formatTime(cycle.started_at)}</div>
        <div class="type-body-sm dur">{formatDuration(duration)}</div>
      </div>

      <TriggerChip trigger={cycle.trigger} />

      <div class="summary">
        {#if isQuiet}
          <span class="type-body quiet-text">
            Scanned {cycle.scanned} containers — all up to date
          </span>
        {:else}
          <span class="type-body">
            {#if cycle.updated > 0}<b>{cycle.updated}</b> updated{/if}
            {#if cycle.updated > 0 && (cycle.rolled_back > 0 || cycle.failed > 0)},
            {/if}
            {#if cycle.rolled_back > 0}<b class="warn">{cycle.rolled_back}</b> rolled back{/if}
            {#if cycle.rolled_back > 0 && cycle.failed > 0},
            {/if}
            {#if cycle.failed > 0}<b class="err">{cycle.failed}</b> failed{/if}
            <span class="muted"> · {cycle.up_to_date} unchanged</span>
          </span>
        {/if}
      </div>

      {#if !isQuiet && !isExpanded && previewContainers.length > 0}
        <div class="preview-chips">
          {#each previewContainers as cc (cc.name)}
            <span class="chip preview-chip">
              <span
                class="preview-dot"
                style="background: {cc.outcome === 'failed'
                  ? 'var(--error)'
                  : cc.outcome === 'rolled_back'
                    ? 'var(--warning)'
                    : 'var(--success)'}"
              ></span>
              {cc.name}
            </span>
          {/each}
        </div>
      {/if}

      <button
        class="btn icon small expand-btn"
        class:rotated={isExpanded}
        onclick={(e) => {
          e.stopPropagation();
          onToggle();
        }}
      >
        <span class="ms">expand_more</span>
      </button>
    </div>

    {#if isExpanded}
      <div class="expanded-panel">
        <div class="meta-row">
          <span><b>#{cycle.id}</b> · {formatAbs(cycle.started_at)}</span>
          <span>scanned {cycle.scanned}</span>
          <span>duration {formatDuration(duration)}</span>
          <span>completed {formatTime(cycle.completed_at)}</span>
        </div>
        <div class="card outlined container-list">
          {#each nonUpToDate as container (container.name)}
            <ContainerOutcomeRow {container} />
          {/each}
          {#if upToDateCount > 0}
            <div class="up-to-date-footer">
              + {upToDateCount} container{upToDateCount === 1 ? '' : 's'} up to date
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .event {
    position: relative;
    padding-left: 48px;
    padding-bottom: 8px;
  }

  .event.quiet {
    opacity: 0.85;
  }

  .dot {
    position: absolute;
    left: 7px;
    top: 12px;
    width: 10px;
    height: 10px;
    border-radius: 999px;
    box-shadow: 0 0 0 3px var(--bg);
  }

  .event-card {
    background: var(--sc-low);
    padding: 14px 18px;
    cursor: pointer;
    transition: background 0.12s;
  }

  .event-card.expanded {
    background: var(--sc);
    box-shadow: var(--e2);
  }

  .event-card.condensed {
    padding: 10px 16px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .time-col {
    width: 64px;
    flex: 0 0 64px;
  }

  .time {
    font-size: 14px;
    font-weight: 600;
  }

  .dur {
    color: var(--on-surface-muted);
    font-size: 11px;
  }

  .summary {
    flex: 1;
    min-width: 0;
  }

  .quiet-text {
    color: var(--on-surface-variant);
  }

  .muted {
    color: var(--on-surface-muted);
  }

  .warn {
    color: var(--warning);
  }

  .err {
    color: var(--error);
  }

  .preview-chips {
    display: flex;
    gap: 4px;
  }

  @media (max-width: 899px) {
    .preview-chips {
      display: none;
    }
  }

  .preview-chip {
    background: var(--surface-bright);
    color: var(--on-surface);
    height: 22px;
    padding: 0 8px;
    font-size: 11px;
    gap: 5px;
  }

  .preview-dot {
    width: 6px;
    height: 6px;
    border-radius: 999px;
  }

  .expand-btn {
    flex-shrink: 0;
    transition: transform 0.15s;
  }

  .expand-btn.rotated {
    transform: rotate(180deg);
  }

  .expanded-panel {
    margin-top: 14px;
    margin-left: 78px;
    border-top: 1px solid var(--outline-variant);
    padding-top: 12px;
  }

  .meta-row {
    display: flex;
    gap: 18px;
    margin-bottom: 10px;
    color: var(--on-surface-variant);
    font-size: 12px;
    flex-wrap: wrap;
  }

  .container-list {
    background: var(--surface);
  }

  .up-to-date-footer {
    padding: 8px 16px;
    color: var(--on-surface-muted);
    font-size: 12px;
    font-style: italic;
  }
</style>
