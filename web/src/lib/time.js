export function formatRelative(isoStr) {
  const diffMs = Date.now() - new Date(isoStr).getTime();
  const mins = Math.floor(diffMs / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  const days = Math.floor(hrs / 24);
  return `${days}d ago`;
}

export function formatAbs(isoStr) {
  return new Date(isoStr).toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export function formatDuration(seconds) {
  if (seconds < 60) return `${seconds}s`;
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return s > 0 ? `${m}m ${s}s` : `${m}m`;
}

export function formatTime(isoStr) {
  return new Date(isoStr).toLocaleTimeString(undefined, {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  });
}

export function groupByDay(list) {
  const groups = [];
  const todayStart = new Date();
  todayStart.setHours(0, 0, 0, 0);
  const todayMs = todayStart.getTime();

  for (const c of list) {
    const cycleDay = new Date(c.started_at);
    cycleDay.setHours(0, 0, 0, 0);
    const diff = Math.round((todayMs - cycleDay.getTime()) / 86400000);
    const label =
      diff === 0
        ? 'Today'
        : diff === 1
          ? 'Yesterday'
          : diff < 7
            ? new Date(c.started_at).toLocaleDateString(undefined, { weekday: 'long' })
            : new Date(c.started_at).toLocaleDateString(undefined, {
                weekday: 'short',
                month: 'short',
                day: 'numeric',
              });

    const grp = groups.find((g) => g.label === label);
    if (grp) grp.cycles.push(c);
    else groups.push({ label, cycles: [c] });
  }
  return groups;
}

export function getDailyAggregates(allCycles) {
  const now = new Date();
  return Array.from({ length: 7 }, (_, i) => {
    const day = new Date(now);
    day.setDate(day.getDate() - (6 - i));
    day.setHours(0, 0, 0, 0);
    const nextDay = new Date(day);
    nextDay.setDate(nextDay.getDate() + 1);
    const dayCycles = allCycles.filter((c) => {
      const t = new Date(c.started_at);
      return t >= day && t < nextDay;
    });
    return {
      updated: dayCycles.reduce((a, c) => a + Number(c.updated), 0),
      failed: dayCycles.reduce((a, c) => a + Number(c.failed), 0),
      rolled_back: dayCycles.reduce((a, c) => a + Number(c.rolled_back), 0),
    };
  });
}
