<script>
  let { trigger } = $props();

  const META = {
    scheduled: { icon: 'schedule', label: 'scheduled' },
    manual: { icon: 'touch_app', label: 'manual' },
    http_api: { icon: 'webhook', label: 'webhook' },
  };

  let m = $derived(META[trigger] ?? META.scheduled);
</script>

<span class="trigger-chip">
  <span class="ms" style="font-size: 16px">{m.icon}</span>
  <span class="label">{m.label}</span>
</span>

<style>
  .trigger-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--on-surface-variant);
    font-size: 13px;
  }

  .label {
    font-variant: small-caps;
    letter-spacing: 0.04em;
    font-weight: 500;
  }
</style>
