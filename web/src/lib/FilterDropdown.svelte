<script>
  let { icon, options = [], value = $bindable('') } = $props();

  let open = $state(false);
  let wrapEl = $state(null);

  $effect(() => {
    if (!open) return;
    function handler(e) {
      if (wrapEl && !wrapEl.contains(e.target)) open = false;
    }
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  });
</script>

<div bind:this={wrapEl} class="filter-wrap">
  <button class="filter-btn" class:open onclick={() => (open = !open)}>
    <span class="ms icon">{icon}</span>
    {value}
    <span class="ms arrow" class:rotated={open}>arrow_drop_down</span>
  </button>

  {#if open}
    <div class="dropdown card">
      {#each options as opt (opt)}
        <button
          class="opt"
          class:active={opt === value}
          onclick={() => {
            value = opt;
            open = false;
          }}
        >
          <span
            class="ms check-icon"
            style="color: {opt === value ? 'var(--primary)' : 'transparent'}"
          >
            check
          </span>
          {opt}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .filter-wrap {
    position: relative;
  }

  .filter-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 36px;
    padding: 0 10px 0 12px;
    border-radius: var(--r-full);
    background: var(--sc-low);
    color: var(--on-surface);
    border: 0;
    font: inherit;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.12s;
  }

  .filter-btn.open {
    background: var(--sc);
  }

  .icon {
    font-size: 16px;
    color: var(--on-surface-variant);
  }

  .arrow {
    font-size: 18px;
    color: var(--on-surface-variant);
    transition: transform 0.15s;
  }

  .arrow.rotated {
    transform: rotate(180deg);
  }

  .dropdown {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    min-width: 180px;
    padding: 4px;
    z-index: 10;
    background: var(--surface);
    box-shadow: var(--e2);
    border: 1px solid var(--outline-variant);
  }

  .opt {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 36px;
    padding: 0 12px;
    border-radius: 6px;
    background: transparent;
    color: var(--on-surface);
    border: 0;
    cursor: pointer;
    font: inherit;
    font-size: 13px;
    text-align: left;
    transition: background 0.1s;
  }

  .opt.active {
    background: var(--primary-soft);
  }

  .opt:not(.active):hover {
    background: var(--sc-low);
  }

  .check-icon {
    font-size: 16px;
    flex-shrink: 0;
  }
</style>
