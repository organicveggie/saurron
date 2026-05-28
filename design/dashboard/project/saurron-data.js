/* Mock cycle history for a Saurron homelab.
   Exposes window.SAURRON_DATA = { cycles, containers } and helpers. */

(function () {
  // --- Homelab container catalog -------------------------------------------
  const CATALOG = [
    { name: 'plex',                 repo: 'plexinc/pms-docker',          tags: ['1.40.2.8395', '1.40.3.8555', '1.40.4.8679'] },
    { name: 'jellyfin',             repo: 'jellyfin/jellyfin',           tags: ['10.9.7', '10.9.8', '10.9.9', '10.9.10'] },
    { name: 'sonarr',               repo: 'lscr.io/linuxserver/sonarr',  tags: ['4.0.8.2105', '4.0.9.2244', '4.0.10.2544'] },
    { name: 'radarr',               repo: 'lscr.io/linuxserver/radarr',  tags: ['5.7.0.8882', '5.8.3.8933', '5.9.1.9070'] },
    { name: 'prowlarr',             repo: 'lscr.io/linuxserver/prowlarr',tags: ['1.21.2.4649', '1.22.0.4670'] },
    { name: 'bazarr',               repo: 'lscr.io/linuxserver/bazarr',  tags: ['1.4.3', '1.4.4', '1.4.5'] },
    { name: 'qbittorrent',          repo: 'lscr.io/linuxserver/qbittorrent', tags: ['4.6.5', '5.0.0', '5.0.2'] },
    { name: 'homeassistant',        repo: 'homeassistant/home-assistant',tags: ['2026.4.3', '2026.5.0', '2026.5.2'] },
    { name: 'mosquitto',            repo: 'eclipse-mosquitto',           tags: ['2.0.18', '2.0.19', '2.0.20'] },
    { name: 'zigbee2mqtt',          repo: 'koenkk/zigbee2mqtt',          tags: ['1.40.1', '1.40.2', '2.0.0'] },
    { name: 'pihole',               repo: 'pihole/pihole',               tags: ['2024.07.0', '2024.09.0', '2025.01.0'] },
    { name: 'unbound',              repo: 'mvance/unbound',              tags: ['1.20.0', '1.21.0'] },
    { name: 'traefik',              repo: 'traefik',                     tags: ['v3.1.4', 'v3.1.6', 'v3.2.0'] },
    { name: 'portainer',            repo: 'portainer/portainer-ce',      tags: ['2.21.3', '2.21.4', '2.22.0'] },
    { name: 'nextcloud',            repo: 'nextcloud',                   tags: ['29.0.6', '29.0.7', '30.0.0'] },
    { name: 'vaultwarden',          repo: 'vaultwarden/server',          tags: ['1.32.1', '1.32.2', '1.32.4'] },
    { name: 'immich-server',        repo: 'ghcr.io/immich-app/immich-server', tags: ['v1.118.2', 'v1.119.0', 'v1.120.1'] },
    { name: 'immich-machine-learning', repo: 'ghcr.io/immich-app/immich-machine-learning', tags: ['v1.118.2', 'v1.119.0', 'v1.120.1'] },
    { name: 'postgres',             repo: 'postgres',                    tags: ['16.4-alpine', '16.5-alpine', '16.6-alpine'] },
    { name: 'redis',                repo: 'redis',                       tags: ['7.4.0-alpine', '7.4.1-alpine'] },
    { name: 'paperless-ngx',        repo: 'ghcr.io/paperless-ngx/paperless-ngx', tags: ['2.11.6', '2.12.0', '2.13.0'] },
    { name: 'frigate',              repo: 'ghcr.io/blakeblackshear/frigate', tags: ['0.14.1-tensorrt', '0.14.2-tensorrt'] },
    { name: 'uptime-kuma',          repo: 'louislam/uptime-kuma',        tags: ['1.23.13', '1.23.15', '1.23.16'] },
    { name: 'grafana',              repo: 'grafana/grafana-oss',         tags: ['11.2.2', '11.3.0', '11.3.1'] },
    { name: 'prometheus',           repo: 'prom/prometheus',             tags: ['v2.54.1', 'v2.55.0', 'v3.0.0'] },
    { name: 'gitea',                repo: 'gitea/gitea',                 tags: ['1.22.3', '1.22.4', '1.23.0'] },
    { name: 'code-server',          repo: 'lscr.io/linuxserver/code-server', tags: ['4.92.2', '4.93.1', '4.94.0'] },
    { name: 'jellyseerr',           repo: 'fallenbagel/jellyseerr',      tags: ['2.0.1', '2.1.0', '2.1.1'] },
    { name: 'tautulli',             repo: 'lscr.io/linuxserver/tautulli',tags: ['2.14.4', '2.14.5'] },
    { name: 'saurron',              repo: 'ghcr.io/organicveggie/saurron', tags: ['v0.6.3', 'v0.6.4', 'v0.7.0'], isSelf: true },
  ];

  // Deterministic PRNG so re-renders don't shuffle the data
  function mulberry32(seed) {
    return function () {
      seed |= 0; seed = seed + 0x6D2B79F5 | 0;
      let t = Math.imul(seed ^ seed >>> 15, 1 | seed);
      t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t;
      return ((t ^ t >>> 14) >>> 0) / 4294967296;
    };
  }
  const rand = mulberry32(20260421);
  const pick = (arr) => arr[Math.floor(rand() * arr.length)];
  const choice = (...arr) => arr[Math.floor(rand() * arr.length)];

  function digest() {
    const hex = '0123456789abcdef';
    let s = '';
    for (let i = 0; i < 12; i++) s += hex[Math.floor(rand() * 16)];
    return s;
  }

  function nextTag(tags, oldTag) {
    const i = tags.indexOf(oldTag);
    return tags[Math.min(tags.length - 1, i + 1)];
  }

  // --- Cycle generator -----------------------------------------------------
  // 26 cycles spanning ~13 days. Most are scheduled hourly-ish; a couple are
  // manual / http_api triggers. We seed outcomes to give the design plenty
  // of states to show.
  const TRIGGERS = ['scheduled', 'scheduled', 'scheduled', 'scheduled', 'manual', 'http_api'];
  // newest first
  const now = new Date('2026-05-23T14:42:00Z');
  const cycles = [];
  let id = 1042;

  // Scripted outcome distribution per cycle:
  // [updated, rolled_back, failed, skipped, up_to_date, duration_sec]
  const script = [
    { mins:  18, t:'manual',    out:[2, 0, 0, 0, 28], dur: 47 },  // newest — recent manual push
    { mins:  72, t:'scheduled', out:[0, 0, 0, 0, 30], dur: 12 },
    { mins: 132, t:'scheduled', out:[0, 0, 0, 0, 30], dur: 11 },
    { mins: 192, t:'scheduled', out:[1, 0, 0, 0, 29], dur: 28 },
    { mins: 252, t:'scheduled', out:[0, 0, 0, 0, 30], dur: 10 },
    { mins: 312, t:'scheduled', out:[3, 1, 1, 0, 25], dur: 92 },  // bad cycle (highlight)
    { mins: 372, t:'scheduled', out:[0, 0, 0, 0, 30], dur: 11 },
    { mins: 432, t:'http_api',  out:[1, 0, 0, 0, 29], dur: 22 },
    { mins: 492, t:'scheduled', out:[0, 0, 0, 0, 30], dur: 12 },
    { mins: 552, t:'scheduled', out:[2, 0, 0, 0, 28], dur: 41 },
    { mins: 612, t:'scheduled', out:[0, 0, 0, 0, 30], dur: 10 },
    { mins: 672, t:'scheduled', out:[0, 0, 1, 0, 29], dur: 35 },  // 1 fail
    { mins: 732, t:'scheduled', out:[1, 0, 0, 0, 29], dur: 24 },
    { mins: 792, t:'scheduled', out:[0, 0, 0, 0, 30], dur: 11 },
    { mins: 852, t:'manual',    out:[4, 0, 0, 0, 26], dur: 64 },  // mass update
    { mins: 912, t:'scheduled', out:[0, 0, 0, 0, 30], dur: 9 },
    { mins: 972, t:'scheduled', out:[0, 0, 0, 0, 30], dur: 11 },
    { mins:1032, t:'scheduled', out:[1, 1, 0, 0, 28], dur: 58 },  // 1 rollback
    { mins:1092, t:'scheduled', out:[0, 0, 0, 0, 30], dur: 10 },
    { mins:1152, t:'scheduled', out:[0, 0, 0, 0, 30], dur: 11 },
    { mins:1212, t:'scheduled', out:[2, 0, 0, 0, 28], dur: 38 },
    { mins:1272, t:'scheduled', out:[0, 0, 0, 0, 30], dur: 12 },
    { mins:1332, t:'scheduled', out:[0, 0, 0, 0, 30], dur: 10 },
    { mins:1392, t:'scheduled', out:[1, 0, 0, 0, 29], dur: 26 },
    { mins:1452, t:'http_api',  out:[2, 0, 0, 0, 28], dur: 45 },
    { mins:1512, t:'scheduled', out:[0, 0, 0, 0, 30], dur: 11 },
  ];

  // Helper: pick `n` random containers from catalog without repeats
  function sampleCatalog(n) {
    const pool = [...CATALOG];
    const out = [];
    for (let i = 0; i < n && pool.length; i++) {
      const idx = Math.floor(rand() * pool.length);
      out.push(pool.splice(idx, 1)[0]);
    }
    return out;
  }

  // Failure / rollback reason library
  const FAIL_REASONS = [
    'startup-timeout (30s)',
    'healthcheck reported unhealthy',
    'exit code 139 (SIGSEGV)',
    'exit code 1 within 4s',
    'manifest pull failed: 503',
    'image entrypoint not executable',
  ];

  for (const s of script) {
    const startedAt = new Date(now.getTime() - s.mins * 60 * 1000);
    const completedAt = new Date(startedAt.getTime() + s.dur * 1000);
    const [updated, rolled_back, failed, skipped, up_to_date] = s.out;
    const scanned = updated + rolled_back + failed + skipped + up_to_date;

    // Distribute outcomes across a sample
    const sample = sampleCatalog(scanned);
    const containers = [];
    let i = 0;
    const push = (c, outcome, extra) => {
      const oldTag = pick(c.tags.slice(0, -1)) || c.tags[0];
      const newTag = outcome === 'updated' || outcome === 'rolled_back' || outcome === 'failed'
        ? nextTag(c.tags, oldTag)
        : null;
      containers.push({
        name: c.name,
        old_image: `${c.repo}:${oldTag}@sha256:${digest()}`,
        new_image: newTag ? `${c.repo}:${newTag}@sha256:${digest()}` : null,
        outcome,
        ...(extra || {}),
      });
    };
    for (let k = 0; k < updated; k++)      push(sample[i++], 'updated');
    for (let k = 0; k < rolled_back; k++)  push(sample[i++], 'rolled_back', { reason: pick(FAIL_REASONS), duration_ms: 1200 + Math.floor(rand()*8000) });
    for (let k = 0; k < failed; k++)       push(sample[i++], 'failed', { reason: pick(FAIL_REASONS), duration_ms: 1200 + Math.floor(rand()*8000) });
    for (let k = 0; k < skipped; k++)      push(sample[i++], 'skipped', { reason: 'digest-pinned' });
    for (let k = 0; k < up_to_date; k++)   push(sample[i++], 'up_to_date');

    cycles.push({
      id: id--,
      started_at: startedAt.toISOString(),
      completed_at: completedAt.toISOString(),
      trigger: s.t,
      scanned,
      updated, rolled_back, failed, skipped, up_to_date,
      duration_sec: s.dur,
      containers,
    });
  }

  // --- helpers -------------------------------------------------------------
  function fmtRelative(iso, ref = new Date()) {
    const t = new Date(iso).getTime();
    const r = ref.getTime();
    const diff = Math.round((r - t) / 1000);
    if (diff < 60)    return `${diff}s ago`;
    if (diff < 3600)  return `${Math.round(diff/60)}m ago`;
    if (diff < 86400) return `${Math.round(diff/3600)}h ago`;
    if (diff < 86400*7) return `${Math.round(diff/86400)}d ago`;
    return new Date(iso).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }
  function fmtAbs(iso) {
    const d = new Date(iso);
    return d.toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', hour12: false });
  }
  function fmtTime(iso) {
    return new Date(iso).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', hour12: false });
  }
  function fmtDay(iso) {
    return new Date(iso).toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' });
  }
  function fmtDuration(sec) {
    if (sec < 60) return `${sec}s`;
    const m = Math.floor(sec/60), s = sec % 60;
    return s ? `${m}m ${s}s` : `${m}m`;
  }
  function imageTagOnly(ref) {
    if (!ref) return '—';
    // strip leading registry/path, keep tag, drop digest
    const noDigest = ref.split('@')[0];
    const lastSlash = noDigest.lastIndexOf('/');
    return noDigest.slice(lastSlash + 1);
  }
  function imageShortDigest(ref) {
    if (!ref) return '';
    const at = ref.indexOf('@sha256:');
    if (at < 0) return '';
    return ref.slice(at + 8, at + 8 + 7);
  }
  function imageRepo(ref) {
    if (!ref) return '';
    return ref.split('@')[0].split(':').slice(0, -1).join(':');
  }

  // --- summary metrics ----------------------------------------------------
  const lastCycle = cycles[0];
  // Updates this week — sum over cycles in the last 7 days
  const weekAgo = new Date(now.getTime() - 7 * 86400 * 1000);
  const withinWeek = cycles.filter(c => new Date(c.started_at) >= weekAgo);
  const updatesThisWeek = withinWeek.reduce((a, c) => a + c.updated, 0);
  const failuresThisWeek = withinWeek.reduce((a, c) => a + c.failed + c.rolled_back, 0);
  const scansThisWeek = withinWeek.length;

  // Outcome of last cycle for the summary badge
  function summarizeCycleOutcome(c) {
    if (c.failed > 0) return { kind: 'error', label: `${c.failed} failed` };
    if (c.rolled_back > 0) return { kind: 'warning', label: `${c.rolled_back} rolled back` };
    if (c.updated > 0) return { kind: 'success', label: `${c.updated} updated` };
    return { kind: 'neutral', label: 'no changes' };
  }

  // 7-day sparkline data (updates per day, oldest -> newest)
  function dailyAggregate() {
    const buckets = Array.from({ length: 7 }, (_, i) => ({
      day: new Date(now.getTime() - (6 - i) * 86400000),
      updated: 0, failed: 0, rolled_back: 0, cycles: 0,
    }));
    cycles.forEach(c => {
      const t = new Date(c.started_at).getTime();
      const dayIdx = buckets.findIndex(b => {
        const start = new Date(b.day); start.setHours(0,0,0,0);
        const end = new Date(start.getTime() + 86400000);
        return t >= start.getTime() && t < end.getTime();
      });
      if (dayIdx >= 0) {
        buckets[dayIdx].updated += c.updated;
        buckets[dayIdx].failed += c.failed;
        buckets[dayIdx].rolled_back += c.rolled_back;
        buckets[dayIdx].cycles += 1;
      }
    });
    return buckets;
  }

  window.SAURRON_DATA = {
    cycles,
    catalog: CATALOG,
    summary: {
      lastCycle,
      lastCycleOutcome: summarizeCycleOutcome(lastCycle),
      updatesThisWeek,
      failuresThisWeek,
      scansThisWeek,
      now,
    },
    daily: dailyAggregate(),
    helpers: {
      fmtRelative, fmtAbs, fmtTime, fmtDay, fmtDuration,
      imageTagOnly, imageShortDigest, imageRepo, summarizeCycleOutcome,
    },
  };
})();
