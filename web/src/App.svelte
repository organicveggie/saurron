<script>
  import Router, { router } from 'svelte-spa-router';
  import { theme } from './stores/theme.js';
  import NavDrawer from './lib/NavDrawer.svelte';
  import TopAppBar from './lib/TopAppBar.svelte';
  import BottomNavBar from './lib/BottomNavBar.svelte';
  import Dashboard from './routes/Dashboard.svelte';
  import Update from './routes/Update.svelte';
  import Template from './routes/Template.svelte';
  import Notifications from './routes/Notifications.svelte';
  import './tokens.css';

  const routes = {
    '/': Dashboard,
    '/update': Update,
    '/template': Template,
    '/notifications': Notifications,
  };

  const PAGE_TITLES = {
    '/': 'Dashboard',
    '/update': 'Manual update',
    '/template': 'Template',
    '/notifications': 'Notifications',
  };

  function getVariant(w) {
    if (w >= 900) return 'standard';
    if (w >= 600) return 'rail';
    return 'hidden';
  }

  let drawerVariant = getVariant(window.innerWidth);

  function handleResize() {
    drawerVariant = getVariant(window.innerWidth);
  }
</script>

<svelte:window onresize={handleResize} />

<div class="saurron-app" data-theme={$theme}>
  <div class="app-shell">
    {#if drawerVariant !== 'hidden'}
      <NavDrawer variant={drawerVariant} />
    {/if}
    <div class="main-col">
      <TopAppBar title={PAGE_TITLES[router.location] ?? 'Saurron'} />
      <main class="page-content">
        <Router {routes} />
      </main>
    </div>
  </div>
  <BottomNavBar />
</div>

<style>
  .app-shell {
    display: flex;
    height: 100dvh;
    overflow: hidden;
  }

  .main-col {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
  }

  .page-content {
    flex: 1;
    overflow-y: auto;
  }

  @media (max-width: 599px) {
    .page-content {
      padding-bottom: 64px;
    }
  }
</style>
