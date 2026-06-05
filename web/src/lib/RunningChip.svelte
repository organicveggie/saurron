<script>
  let { running = false } = $props();
</script>

<span class="running-chip {running ? 'live' : 'idle'}">
  <span class="dot"></span>
  {running ? 'running' : 'idle'}
</span>
