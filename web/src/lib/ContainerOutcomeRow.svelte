<script>
  import { imageTagOnly, imageShortDigest } from './image.js';
  import OutcomeChip from './atoms/OutcomeChip.svelte';

  let { container } = $props();

  let oldTag = $derived(imageTagOnly(container.old_image));
  let oldDigest = $derived(imageShortDigest(container.old_image));
  let newTag = $derived(imageTagOnly(container.new_image));
  let newDigest = $derived(imageShortDigest(container.new_image));

  const DOT_COLOR = {
    failed: 'var(--error)',
    rolled_back: 'var(--warning)',
    updated: 'var(--success)',
    skipped: 'var(--neutral)',
    up_to_date: 'var(--neutral)',
  };
</script>

<div class="row">
  <div class="name-col">
    <span class="dot" style="background: {DOT_COLOR[container.outcome] ?? 'var(--neutral)'}"></span>
    <span class="type-mono name">{container.name}</span>
  </div>

  <div class="type-mono image-ref">
    {oldTag}
    {#if oldDigest}
      <span class="digest">@{oldDigest}</span>
    {/if}
  </div>

  <span class="ms arrow-icon">arrow_forward</span>

  <div class="type-mono image-ref" class:muted={!container.new_image}>
    {#if container.new_image}
      {newTag}
      {#if newDigest}
        <span class="digest">@{newDigest}</span>
      {/if}
    {:else}
      {container.reason ?? '—'}
    {/if}
  </div>

  <div class="chip-col">
    <OutcomeChip outcome={container.outcome} dense />
  </div>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 180px 1fr 24px 1fr 140px;
    align-items: center;
    gap: 12px;
    padding: 6px 16px;
    border-bottom: 1px solid var(--outline-variant);
    font-size: 13px;
  }

  .row:last-child {
    border-bottom: none;
  }

  .name-col {
    display: flex;
    align-items: center;
    gap: 8px;
    overflow: hidden;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 999px;
    flex-shrink: 0;
  }

  .name {
    font-weight: 600;
    color: var(--on-surface);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .image-ref {
    color: var(--on-surface-variant);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .image-ref.muted {
    color: var(--on-surface-muted);
  }

  .digest {
    color: var(--on-surface-muted);
    margin-left: 6px;
  }

  .arrow-icon {
    color: var(--on-surface-muted);
    font-size: 16px;
    text-align: center;
  }

  .chip-col {
    justify-self: end;
  }
</style>
