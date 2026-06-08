export const containers = [
  {
    name: 'web',
    state: 'running',
    image: 'ghcr.io/example/web:1.4.0',
  },
  {
    name: 'db',
    state: 'pinned',
    image: 'postgres:16@sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789',
  },
  {
    name: 'metrics',
    state: 'monitor_only',
    image: 'prom/prometheus:v2.53.0',
  },
];
