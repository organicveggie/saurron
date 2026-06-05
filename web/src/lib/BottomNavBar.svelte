<script>
  import { router, push } from 'svelte-spa-router';

  const NAV_ITEMS = [
    { route: '/', label: 'Dashboard', icon: 'dashboard' },
    { route: '/update', label: 'Manual update', icon: 'play_circle' },
    { route: '/template', label: 'Template', icon: 'description' },
    { route: '/notifications', label: 'Notifications', icon: 'notifications' },
  ];
</script>

<nav class="bottom-nav">
  {#each NAV_ITEMS as item (item.route)}
    <button
      class="bottom-nav-item"
      class:active={router.location === item.route}
      on:click={() => push(item.route)}
    >
      <span class="ms" class:fill={router.location === item.route} style="font-size: 24px"
        >{item.icon}</span
      >
      <span class="bottom-nav-label">{item.label}</span>
    </button>
  {/each}
</nav>

<style>
  .bottom-nav {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    height: 64px;
    display: flex;
    align-items: stretch;
    background: var(--surface);
    border-top: 1px solid var(--outline-variant);
    z-index: 100;
  }

  @media (min-width: 600px) {
    .bottom-nav {
      display: none;
    }
  }

  .bottom-nav-item {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    background: transparent;
    border: none;
    cursor: pointer;
    color: var(--on-surface-variant);
    font: inherit;
    padding: 8px 4px;
    transition: color 0.12s;
    min-width: 0;
  }

  .bottom-nav-item.active {
    color: var(--primary);
  }

  .bottom-nav-label {
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.02em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
</style>
