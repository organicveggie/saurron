<script>
  import { push } from 'svelte-spa-router';
  import { health } from '../stores/health.js';
  import { theme } from '../stores/theme.js';

  let { title = '', subtitle = null } = $props();

  function toggleTheme() {
    theme.update((t) => (t === 'dark' ? 'light' : 'dark'));
  }
</script>

<header class="top-bar">
  <div class="top-bar-title">
    <div class="type-h1">{title}</div>
    {#if subtitle}
      <div class="type-body-sm" style="color: var(--on-surface-muted)">{subtitle}</div>
    {/if}
  </div>

  <div class="search-field">
    <span class="ms" style="font-size: 18px">search</span>
    <input placeholder="Search cycles, containers…" />
    <kbd>⌘K</kbd>
  </div>

  <button class="btn icon" title="Toggle theme" onclick={toggleTheme}>
    <span class="ms">{$theme === 'dark' ? 'light_mode' : 'dark_mode'}</span>
  </button>

  {#if $health.updating}
    <button class="btn tonal run-btn" disabled>
      <span class="running-dot" style="width: 8px; height: 8px"></span>
      Running…
    </button>
  {:else}
    <button class="btn filled run-btn" onclick={() => push('/update')}>
      <span class="ms">play_arrow</span>
      Run cycle
    </button>
  {/if}

  {#if $health.updating}
    <div class="running-bar thin progress-underline"></div>
  {/if}
</header>

<style>
  .top-bar {
    position: relative;
    height: 72px;
    flex: 0 0 72px;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 24px;
    background: var(--surface);
    border-bottom: 1px solid var(--outline-variant);
  }

  .top-bar-title {
    flex: 1;
    display: flex;
    align-items: baseline;
    gap: 14px;
    min-width: 0;
  }

  .search-field {
    position: relative;
    display: flex;
    align-items: center;
    height: 40px;
    width: 280px;
    background: var(--sc);
    border-radius: var(--r-full);
    padding: 0 14px;
    gap: 10px;
    color: var(--on-surface-variant);
  }

  .search-field input {
    border: none;
    outline: none;
    background: transparent;
    color: var(--on-surface);
    font: inherit;
    font-size: 13px;
    flex: 1;
  }

  .run-btn {
    height: 40px;
    white-space: nowrap;
  }

  .progress-underline {
    position: absolute;
    left: 0;
    right: 0;
    bottom: -1px;
    width: 100%;
    background: transparent;
  }
</style>
