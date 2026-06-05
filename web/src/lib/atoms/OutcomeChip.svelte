<script>
  export let outcome;
  export let dense = false;

  const META = {
    updated: { cls: 'success', icon: 'check_circle', label: 'updated' },
    rolled_back: { cls: 'warning', icon: 'undo', label: 'rolled back' },
    failed: { cls: 'error', icon: 'error', label: 'failed' },
    skipped: { cls: 'neutral', icon: 'skip_next', label: 'skipped' },
    up_to_date: { cls: 'neutral', icon: 'check', label: 'up-to-date' },
  };

  $: m = META[outcome] ?? META.up_to_date;
</script>

<span class="chip {m.cls}" style={dense ? 'height: 22px; padding: 0 8px; font-size: 11px;' : ''}>
  <span class="ms" style={dense ? 'font-size: 13px;' : 'font-size: 14px;'}>{m.icon}</span>
  {m.label}
</span>
