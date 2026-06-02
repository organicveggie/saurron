<script>
  import { writable } from 'svelte/store';
  import Router from 'svelte-spa-router';
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

  const theme = writable(document.documentElement.dataset.theme ?? 'light');

  theme.subscribe((t) => {
    document.documentElement.dataset.theme = t;
    localStorage.setItem('theme', t);
  });
</script>

<div class="saurron-app" data-theme={$theme}>
  <Router {routes} />
</div>
