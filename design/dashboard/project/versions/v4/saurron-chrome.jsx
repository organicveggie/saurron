// Shared chrome: NavDrawer + TopAppBar + small atoms used by all 3 layouts.
// Exports: window.{NavDrawer, TopAppBar, OutcomeChip, TriggerChip, ContainerOutcomeRow, Sparkline, useTheme}

const SaurronLogo = ({ size = 28 }) => (
  // Saurron eye logomark — simplified from the project SVG.
  // A vertical almond eye with a pupil; renders as the primary color.
  <svg width={size} height={size} viewBox="0 0 40 40" aria-label="Saurron" fill="none">
    <path
      d="M20 6 C 30 6, 36 14, 38 20 C 36 26, 30 34, 20 34 C 10 34, 4 26, 2 20 C 4 14, 10 6, 20 6 Z"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.5"
      strokeLinejoin="round"
    />
    <ellipse cx="20" cy="20" rx="4" ry="9" fill="currentColor" />
    <ellipse cx="20" cy="20" rx="1.5" ry="5" fill="var(--surface)" />
  </svg>
);

function useTheme() {
  // The theme is set on document.body[data-theme] by the host tweaks panel.
  // Components don't need to know it (CSS vars handle the swap) — this hook
  // exists so the top app bar's toggle button can flip the data attr.
  const [theme, setTheme] = React.useState(() => document.body.getAttribute('data-theme') || 'dark');
  React.useEffect(() => {
    const obs = new MutationObserver(() => {
      setTheme(document.body.getAttribute('data-theme') || 'dark');
    });
    obs.observe(document.body, { attributes: true, attributeFilter: ['data-theme'] });
    return () => obs.disconnect();
  }, []);
  const toggle = () => {
    const next = theme === 'dark' ? 'light' : 'dark';
    document.body.setAttribute('data-theme', next);
    // Also broadcast to the host tweaks editor so the toggle persists.
    try {
      window.parent.postMessage({ type: '__edit_mode_set_keys', edits: { theme: next } }, '*');
    } catch {}
  };
  return [theme, toggle];
}

// ============================================================
// Nav drawer (left, persistent)
// ============================================================
function NavDrawer({ active = 'dashboard', variant = 'standard', running = null }) {
  // `running`: null/undefined = idle. Object = live cycle:
  //   { startedAt: Date|number, scanned: n, total: n, current: 'name' }
  // variant: 'standard' (wide) | 'rail' (collapsed icons only) | 'compact' (narrow w/ icons + labels)
  const items = [
    { id: 'dashboard',    label: 'Dashboard',     icon: 'dashboard',         badge: null },
    { id: 'update',       label: 'Manual update', icon: 'play_circle',       badge: null },
    { id: 'template',     label: 'Template',      icon: 'description',       badge: null },
    { id: 'notifications',label: 'Notifications', icon: 'notifications',     badge: null },
  ];
  const containers = [
    { name: 'plex',           state: 'running' },
    { name: 'jellyfin',       state: 'running' },
    { name: 'homeassistant',  state: 'running' },
    { name: 'sonarr',         state: 'monitor-only' },
    { name: 'radarr',         state: 'monitor-only' },
    { name: 'frigate',        state: 'pinned' },
  ];

  if (variant === 'rail') {
    return (
      <nav style={{
        width: 64, flex: '0 0 64px',
        background: 'var(--surface)',
        borderRight: '1px solid var(--outline-variant)',
        display: 'flex', flexDirection: 'column', alignItems: 'center',
        padding: '14px 0 10px', gap: 6,
      }}>
        <div style={{
          color: 'var(--primary)',
          position: 'relative',
          marginBottom: 12,
        }}>
          <SaurronLogo size={28} />
          {/* Tiny running-dot tucked into the upper-right of the logomark */}
          {running && (
            <span className="running-dot" style={{
              position: 'absolute', top: -2, right: -2,
              boxShadow: '0 0 0 2px var(--surface)',
            }} />
          )}
        </div>
        <div style={{ width: 28, height: 1, background: 'var(--outline-variant)', marginBottom: 6 }} />
        {items.map(it => {
          const isActive = it.id === active;
          return (
            <button key={it.id} className="btn icon" title={it.label}
              style={{
                width: 48, height: 48, borderRadius: 14,
                background: isActive ? 'var(--secondary-container)' : 'transparent',
                color: isActive ? 'var(--on-secondary-container)' : 'var(--on-surface-variant)',
              }}>
              <span className={"ms" + (isActive ? ' fill' : '')} style={{ fontSize: 24 }}>{it.icon}</span>
            </button>
          );
        })}
        <div style={{ flex: 1 }} />
        {/* Cycle status badge — circular progress ring when running, "in 42m" pill when idle */}
        <CycleStatusBadge running={running} />
      </nav>
    );
  }

  return (
    <nav style={{
      width: 264, flex: '0 0 264px',
      background: 'var(--surface)',
      borderRight: '1px solid var(--outline-variant)',
      display: 'flex', flexDirection: 'column',
      padding: '14px 12px',
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '6px 8px 18px' }}>
        <div style={{ color: 'var(--primary)' }}><SaurronLogo size={28} /></div>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <div className="type-h2" style={{ letterSpacing: '-0.01em', lineHeight: 1 }}>Saurron</div>
            {/* #1 — drawer header status chip */}
            <RunningChip running={running} />
          </div>
          <div className="type-mono" style={{ color: 'var(--on-surface-muted)', fontSize: 11, marginTop: 4 }}>v0.7.0 · homelab-01</div>
        </div>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
        {items.map(it => {
          const isActive = it.id === active;
          return (
            <button key={it.id}
              style={{
                display: 'flex', alignItems: 'center', gap: 12,
                height: 48, padding: '0 16px',
                borderRadius: 999, border: 0,
                background: isActive ? 'var(--secondary-container)' : 'transparent',
                color: isActive ? 'var(--on-secondary-container)' : 'var(--on-surface-variant)',
                cursor: 'pointer', textAlign: 'left',
                font: 'inherit', fontWeight: isActive ? 600 : 500, fontSize: 14,
                transition: 'background .12s',
              }}>
              <span className={"ms" + (isActive ? ' fill' : '')} style={{ fontSize: 22 }}>{it.icon}</span>
              <span style={{ flex: 1 }}>{it.label}</span>
              {it.badge ? <span className="chip primary" style={{ height: 20, fontSize: 11 }}>{it.badge}</span> : null}
            </button>
          );
        })}
      </div>

      <div style={{ marginTop: 22, padding: '0 8px' }}>
        <div className="type-overline" style={{ fontSize: 11 }}>Watched</div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 2, marginTop: 8 }}>
          {containers.map(c => (
            <div key={c.name} style={{
              display: 'flex', alignItems: 'center', gap: 8,
              padding: '7px 8px', borderRadius: 8,
              color: 'var(--on-surface-variant)', fontSize: 13,
            }}>
              <span style={{
                width: 8, height: 8, borderRadius: 999,
                background: c.state === 'running' ? 'var(--success)'
                  : c.state === 'monitor-only' ? 'var(--info)'
                  : 'var(--neutral)',
              }} />
              <span className="type-mono" style={{ flex: 1, fontSize: 12 }}>{c.name}</span>
              {c.state === 'monitor-only' && <span className="ms" title="monitor-only" style={{ fontSize: 14, color: 'var(--on-surface-muted)' }}>visibility</span>}
              {c.state === 'pinned' && <span className="ms" title="digest pinned" style={{ fontSize: 14, color: 'var(--on-surface-muted)' }}>lock</span>}
            </div>
          ))}
          <button style={{
            background: 'transparent', border: 0, font: 'inherit', cursor: 'pointer',
            color: 'var(--primary)', textAlign: 'left', padding: '6px 8px', fontSize: 12, fontWeight: 600,
          }}>+ 24 more…</button>
        </div>
      </div>

      <div style={{ flex: 1 }} />

      {/* #2 — "Next cycle" card morphs into the running-cycle progress card */}
      <CycleStatusCard running={running} />
    </nav>
  );
}

// ============================================================
// Cycle status chip (drawer header) — #1
// ============================================================
function RunningChip({ running }) {
  if (!running) {
    return (
      <span className="running-chip idle" title="No cycle running">
        <span className="dot" />idle
      </span>
    );
  }
  return (
    <span className="running-chip live" title="Cycle in progress">
      <span className="dot" />running
    </span>
  );
}

// ============================================================
// Cycle status card (drawer footer) — #2
// ============================================================
function CycleStatusCard({ running }) {
  if (!running) {
    return (
      <div className="card outlined" style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 8 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span className="ms" style={{ color: 'var(--success)', fontSize: 18 }}>schedule</span>
          <div className="type-label" style={{ color: 'var(--on-surface)' }}>Next cycle</div>
        </div>
        <div className="type-mono type-num" style={{ fontSize: 18, fontWeight: 600 }}>in 42m</div>
        <div className="type-body-sm" style={{ color: 'var(--on-surface-muted)' }}>every 1h · 30 watched</div>
      </div>
    );
  }
  const { scanned, total, current, elapsed } = running;
  const pct = total > 0 ? Math.round((scanned / total) * 100) : 0;
  return (
    <div className="card outlined" style={{
      padding: 12, display: 'flex', flexDirection: 'column', gap: 10,
      background: 'var(--primary-soft)',
      boxShadow: 'inset 0 0 0 1px color-mix(in oklch, var(--primary) 30%, transparent)',
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <span className="running-dot" />
        <div className="type-label" style={{ color: 'var(--primary)' }}>Cycle running</div>
        <div style={{ flex: 1 }} />
        <span className="type-mono type-num" style={{ fontSize: 11, color: 'var(--on-surface-muted)' }}>{elapsed}</span>
      </div>
      <div className="running-bar" />
      <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', gap: 8 }}>
        <div className="type-mono" style={{ fontSize: 12, color: 'var(--on-surface)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', flex: 1 }}>
          {current}
        </div>
        <div className="type-mono type-num" style={{ fontSize: 12, fontWeight: 600, color: 'var(--on-surface)' }}>
          {scanned}/{total}
        </div>
      </div>
      <div className="type-body-sm" style={{ color: 'var(--on-surface-muted)', fontSize: 11 }}>
        scanning · {pct}% complete
      </div>
    </div>
  );
}

// ============================================================
// Cycle status badge (rail variant of #2) — compact circular badge
// ============================================================
function CycleStatusBadge({ running }) {
  if (!running) {
    return (
      <div title="Next cycle in 42m" style={{
        display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 2,
        padding: '6px 6px 4px',
        borderRadius: 12,
      }}>
        <span className="ms" style={{ fontSize: 18, color: 'var(--on-surface-variant)' }}>schedule</span>
        <span className="type-mono type-num" style={{ fontSize: 10, fontWeight: 600, color: 'var(--on-surface)' }}>42m</span>
      </div>
    );
  }
  const { scanned, total } = running;
  const pct = total > 0 ? scanned / total : 0;
  // SVG ring: 24 radius, 2.5 stroke
  const r = 18, c = 2 * Math.PI * r;
  return (
    <div title={`Cycle running · ${scanned}/${total}`} style={{
      display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 2,
      padding: '6px 6px 4px',
    }}>
      <div style={{ position: 'relative', width: 44, height: 44 }}>
        <svg width="44" height="44" viewBox="0 0 44 44" style={{ transform: 'rotate(-90deg)' }}>
          <circle cx="22" cy="22" r={r} fill="none" stroke="var(--primary-soft)" strokeWidth="3" />
          <circle cx="22" cy="22" r={r} fill="none" stroke="var(--primary)" strokeWidth="3"
            strokeLinecap="round"
            strokeDasharray={c}
            strokeDashoffset={c * (1 - pct)}
            style={{ transition: 'stroke-dashoffset .3s' }}
          />
        </svg>
        <div style={{
          position: 'absolute', inset: 0,
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          fontFamily: 'var(--font-mono)',
          fontSize: 10, fontWeight: 700, color: 'var(--primary)',
          fontVariantNumeric: 'tabular-nums',
        }}>
          {scanned}/{total}
        </div>
      </div>
      <span className="type-overline" style={{ fontSize: 9, color: 'var(--primary)' }}>RUNNING</span>
    </div>
  );
}

// ============================================================
// Top app bar
// ============================================================
function TopAppBar({ title = 'Dashboard', subtitle = null, density = 'normal', running = null, compact = false }) {
  const [theme, toggle] = useTheme();
  const h = density === 'compact' ? 56 : 72;
  return (
    <header style={{
      position: 'relative',
      height: h, flex: `0 0 ${h}px`,
      display: 'flex', alignItems: 'center', gap: compact ? 8 : 16,
      padding: compact ? '0 16px' : '0 24px',
      background: 'var(--surface)',
      borderBottom: '1px solid var(--outline-variant)',
    }}>
      <div style={{ flex: 1, display: 'flex', alignItems: 'baseline', gap: 14, minWidth: 0 }}>
        <div className="type-h1" style={{ fontWeight: 600, fontSize: compact ? 18 : 22 }}>{title}</div>
        {subtitle && !compact && <div className="type-body-sm" style={{ color: 'var(--on-surface-muted)' }}>{subtitle}</div>}
      </div>
      {compact ? (
        <button className="btn icon" title="Search">
          <span className="ms">search</span>
        </button>
      ) : (
        <div style={{
          position: 'relative',
          display: 'flex', alignItems: 'center',
          height: 40, width: 280,
          background: 'var(--sc)', borderRadius: 999,
          padding: '0 14px', gap: 10,
          color: 'var(--on-surface-variant)',
        }}>
          <span className="ms" style={{ fontSize: 18 }}>search</span>
          <input placeholder="Search cycles, containers…"
            style={{
              border: 0, outline: 0, background: 'transparent',
              color: 'var(--on-surface)', fontSize: 13, flex: 1,
              font: 'inherit',
            }} />
          <kbd>⌘K</kbd>
        </div>
      )}
      <button className="btn icon" title="Toggle theme" onClick={toggle}>
        <span className="ms">{theme === 'dark' ? 'light_mode' : 'dark_mode'}</span>
      </button>
      {!compact && (
        <button className="btn icon" title="Refresh">
          <span className="ms">refresh</span>
        </button>
      )}
      {/* #3 — Run cycle button morphs to disabled "Running…" pill while a cycle is in flight */}
      {running ? (
        compact ? (
          <button className="btn icon" disabled title={`Cycle running · ${running.scanned}/${running.total}`}
            style={{
              background: 'var(--primary-soft)',
              color: 'var(--primary)',
              boxShadow: 'inset 0 0 0 1px color-mix(in oklch, var(--primary) 30%, transparent)',
              cursor: 'not-allowed',
            }}>
            <span className="running-dot" style={{ width: 10, height: 10 }} />
          </button>
        ) : (
          <button className="btn tonal" disabled title="A cycle is already running"
            style={{
              height: 40, gap: 8, cursor: 'not-allowed',
              background: 'var(--primary-soft)',
              color: 'var(--primary)',
              boxShadow: 'inset 0 0 0 1px color-mix(in oklch, var(--primary) 30%, transparent)',
            }}>
            <span className="running-dot" style={{ width: 8, height: 8 }} />
            Running…
            <span className="type-mono type-num" style={{ fontSize: 11, opacity: 0.75, marginLeft: 2 }}>
              {running.scanned}/{running.total}
            </span>
          </button>
        )
      ) : (
        compact ? (
          <button className="btn filled" style={{ height: 40, padding: '0 16px' }} title="Run cycle">
            <span className="ms">play_arrow</span>
            Run
          </button>
        ) : (
          <button className="btn filled" style={{ height: 40 }}>
            <span className="ms">play_arrow</span>
            Run cycle
          </button>
        )
      )}

      {/* Indeterminate progress underline along the entire app bar while running */}
      {running && (
        <div className="running-bar thin" style={{
          position: 'absolute', left: 0, right: 0, bottom: -1,
          width: '100%', background: 'transparent',
        }} />
      )}
    </header>
  );
}

// ============================================================
// Outcome / trigger chips
// ============================================================
const OUTCOME_META = {
  updated:     { cls: 'success', icon: 'check_circle',      label: 'updated' },
  rolled_back: { cls: 'warning', icon: 'undo',              label: 'rolled back' },
  failed:      { cls: 'error',   icon: 'error',             label: 'failed' },
  skipped:     { cls: 'neutral', icon: 'skip_next',         label: 'skipped' },
  up_to_date:  { cls: 'neutral', icon: 'check',             label: 'up-to-date' },
};
function OutcomeChip({ outcome, withIcon = true, dense = false }) {
  const m = OUTCOME_META[outcome] || OUTCOME_META.up_to_date;
  return (
    <span className={"chip " + m.cls} style={dense ? { height: 22, padding: '0 8px', fontSize: 11 } : null}>
      {withIcon && <span className="ms" style={{ fontSize: dense ? 13 : 14 }}>{m.icon}</span>}
      {m.label}
    </span>
  );
}

const TRIGGER_META = {
  scheduled: { icon: 'schedule',   label: 'scheduled' },
  manual:    { icon: 'touch_app',  label: 'manual' },
  http_api:  { icon: 'webhook',    label: 'webhook' },
};
function TriggerChip({ trigger }) {
  const m = TRIGGER_META[trigger] || TRIGGER_META.scheduled;
  return (
    <span style={{
      display: 'inline-flex', alignItems: 'center', gap: 6,
      color: 'var(--on-surface-variant)', fontSize: 13,
    }}>
      <span className="ms" style={{ fontSize: 16 }}>{m.icon}</span>
      <span style={{ fontVariant: 'small-caps', letterSpacing: '0.04em', fontWeight: 500 }}>{m.label}</span>
    </span>
  );
}

// ============================================================
// Container outcome row (used inside expanded cycles)
// ============================================================
function ContainerOutcomeRow({ c, dense = false }) {
  const H = window.SAURRON_DATA.helpers;
  const oldTag = H.imageTagOnly(c.old_image);
  const newTag = H.imageTagOnly(c.new_image);
  const oldDigest = H.imageShortDigest(c.old_image);
  const newDigest = H.imageShortDigest(c.new_image);
  return (
    <div style={{
      display: 'grid',
      gridTemplateColumns: '180px 1fr 24px 1fr 140px',
      alignItems: 'center', gap: 12,
      padding: dense ? '6px 16px' : '10px 16px',
      borderBottom: '1px solid var(--outline-variant)',
      fontSize: 13,
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <span style={{
          width: 6, height: 6, borderRadius: 999,
          background: c.outcome === 'failed' ? 'var(--error)'
            : c.outcome === 'rolled_back' ? 'var(--warning)'
            : c.outcome === 'updated' ? 'var(--success)'
            : 'var(--neutral)',
        }} />
        <span className="type-mono" style={{ fontWeight: 600, color: 'var(--on-surface)' }}>{c.name}</span>
      </div>
      <div className="type-mono" style={{ color: 'var(--on-surface-variant)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
        {oldTag}
        {oldDigest && <span style={{ color: 'var(--on-surface-muted)', marginLeft: 6 }}>@{oldDigest}</span>}
      </div>
      <span className="ms" style={{ color: 'var(--on-surface-muted)', fontSize: 16, textAlign: 'center' }}>arrow_forward</span>
      <div className="type-mono" style={{ color: c.new_image ? 'var(--on-surface)' : 'var(--on-surface-muted)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
        {c.new_image ? (
          <>
            {newTag}
            {newDigest && <span style={{ color: 'var(--on-surface-muted)', marginLeft: 6 }}>@{newDigest}</span>}
          </>
        ) : (c.reason || '—')}
      </div>
      <div style={{ justifySelf: 'end' }}>
        <OutcomeChip outcome={c.outcome} dense={dense} />
      </div>
    </div>
  );
}

// ============================================================
// Sparkline (compact daily aggregate)
// ============================================================
function Sparkline({ data, w = 120, h = 32, accent = 'var(--primary)' }) {
  const max = Math.max(1, ...data.map(d => d.updated + d.failed + d.rolled_back));
  const step = data.length > 1 ? w / (data.length - 1) : 0;
  const points = data.map((d, i) => {
    const total = d.updated + d.failed + d.rolled_back;
    const y = h - (total / max) * (h - 4) - 2;
    return [i * step, y];
  });
  const path = points.map((p, i) => (i ? 'L' : 'M') + p[0].toFixed(1) + ' ' + p[1].toFixed(1)).join(' ');
  const area = `${path} L ${w} ${h} L 0 ${h} Z`;
  return (
    <svg width={w} height={h} viewBox={`0 0 ${w} ${h}`} style={{ overflow: 'visible' }}>
      <path d={area} fill={accent} opacity="0.18" />
      <path d={path} fill="none" stroke={accent} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
      {points.map((p, i) => {
        const d = data[i];
        const isBad = d.failed + d.rolled_back > 0;
        return (
          <circle key={i} cx={p[0]} cy={p[1]} r={isBad ? 2.5 : 1.5}
            fill={isBad ? 'var(--error)' : accent} />
        );
      })}
    </svg>
  );
}

// Bar chart for ops console — vertical bars stacked updated/failed
function StackedBars({ data, w = 160, h = 56 }) {
  const max = Math.max(2, ...data.map(d => d.updated + d.failed + d.rolled_back));
  const barW = (w / data.length) * 0.65;
  const gap = (w / data.length) * 0.35;
  return (
    <svg width={w} height={h} viewBox={`0 0 ${w} ${h}`}>
      {data.map((d, i) => {
        const x = i * (barW + gap) + gap / 2;
        const upH = (d.updated / max) * (h - 4);
        const failH = ((d.failed + d.rolled_back) / max) * (h - 4);
        const totalH = upH + failH;
        return (
          <g key={i}>
            {totalH === 0 && (
              <rect x={x} y={h - 2} width={barW} height={2} rx={1} fill="var(--outline-variant)" />
            )}
            {upH > 0 && (
              <rect x={x} y={h - upH} width={barW} height={upH} rx={2} fill="var(--primary)" />
            )}
            {failH > 0 && (
              <rect x={x} y={h - upH - failH} width={barW} height={failH} rx={2} fill="var(--error)" />
            )}
          </g>
        );
      })}
    </svg>
  );
}

Object.assign(window, {
  SaurronLogo, NavDrawer, TopAppBar,
  OutcomeChip, TriggerChip, ContainerOutcomeRow,
  Sparkline, StackedBars, useTheme,
});
