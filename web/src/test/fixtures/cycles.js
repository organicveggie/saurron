export const cycles = [
  {
    id: 42,
    started_at: '2026-06-07T12:00:00Z',
    completed_at: '2026-06-07T12:02:30Z',
    trigger: 'scheduled',
    scanned: 3,
    updated: 1,
    failed: 0,
    rolled_back: 0,
    up_to_date: 2,
    containers: [
      {
        name: 'web',
        outcome: 'updated',
        old_image: 'ghcr.io/example/web:1.3.0',
        new_image: 'ghcr.io/example/web:1.4.0',
      },
      { name: 'db', outcome: 'up_to_date' },
      { name: 'metrics', outcome: 'up_to_date' },
    ],
  },
  {
    id: 41,
    started_at: '2026-06-06T12:00:00Z',
    completed_at: '2026-06-06T12:01:10Z',
    trigger: 'manual',
    scanned: 3,
    updated: 0,
    failed: 1,
    rolled_back: 1,
    up_to_date: 1,
    containers: [
      {
        name: 'web',
        outcome: 'failed',
        old_image: 'ghcr.io/example/web:1.3.0',
        reason: 'health check timed out',
      },
      {
        name: 'db',
        outcome: 'rolled_back',
        old_image: 'postgres:15',
        new_image: 'postgres:16',
      },
      { name: 'metrics', outcome: 'up_to_date' },
    ],
  },
  {
    id: 40,
    started_at: '2026-06-05T12:00:00Z',
    completed_at: '2026-06-05T12:00:45Z',
    trigger: 'http_api',
    scanned: 3,
    updated: 0,
    failed: 0,
    rolled_back: 0,
    up_to_date: 3,
    containers: [
      { name: 'web', outcome: 'up_to_date' },
      { name: 'db', outcome: 'up_to_date' },
      { name: 'metrics', outcome: 'skipped', reason: 'monitor-only' },
    ],
  },
];

export const historyPage = (page = 1, perPage = 20) => {
  const start = (page - 1) * perPage;
  return {
    cycles: cycles.slice(start, start + perPage),
    total: cycles.length,
  };
};
