<script>
  export let running = false;
</script>

<span class="running-chip {running ? 'live' : 'idle'}">
  <span class="dot"></span>
  {running ? 'running' : 'idle'}
</span>
