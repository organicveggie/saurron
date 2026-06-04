// Layout 2 — Timeline / feed-driven dashboard.
// Hero "last cycle" card + smaller stat tiles, then a vertical event-stream
// timeline of cycles grouped by day. Each cycle expands inline within the
// timeline. Older cycles load via "Load older" button rather than numbered
// pagination — feels more like an activity feed.

function LayoutTimeline({ data, running = null }) {
  const H = data.helpers;
  const [visibleCount, setVisibleCount] = React.useState(8);
  const [expanded, setExpanded] = React.useState(data.cycles[5]?.id ?? null);
  const visible = data.cycles.slice(0, visibleCount);

  // Group cycles by day label
  const groups = [];
  const refDay = new Date(data.summary.now);refDay.setHours(0, 0, 0, 0);
  visible.forEach((c) => {
    const d = new Date(c.started_at);d.setHours(0, 0, 0, 0);
    const diff = Math.round((refDay - d) / 86400000);
    const label = diff === 0 ? 'Today' :
    diff === 1 ? 'Yesterday' :
    diff < 7 ? new Date(c.started_at).toLocaleDateString(undefined, { weekday: 'long' }) :
    new Date(c.started_at).toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' });
    const grp = groups.find((g) => g.label === label);
    if (grp) grp.cycles.push(c);else groups.push({ label, day: d, cycles: [c] });
  });

  const { lastCycle, lastCycleOutcome, updatesThisWeek, failuresThisWeek, scansThisWeek } = data.summary;

  return (
    <div style={{ flex: 1, display: 'flex', overflow: 'hidden', background: 'var(--bg)' }}>
      <NavDrawer active="dashboard" running={running} />
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
      <TopAppBar title="Dashboard" subtitle="Update history & cycle activity" running={running} />

      <div style={{ flex: 1, overflow: 'auto', padding: '24px 32px 32px' }}>
        {/* Summary cards — same compact row as the orthodox layout */}
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 16, marginBottom: 24 }}>
          <StatCard
              label="Last cycle"
              value={H.fmtRelative(lastCycle.started_at, data.summary.now)}
              sub={`${H.fmtAbs(lastCycle.started_at)} · ${H.fmtDuration(lastCycle.duration_sec)}`}
              badge={<OutcomeChip outcome={
              lastCycleOutcome.kind === 'success' ? 'updated' :
              lastCycleOutcome.kind === 'warning' ? 'rolled_back' :
              lastCycleOutcome.kind === 'error' ? 'failed' :
              'up_to_date'
              } />}
              icon="schedule" />
            
          <StatCard
              label="Updates this week"
              value={updatesThisWeek}
              sub={`across ${scansThisWeek} cycles`}
              chart={<Sparkline data={data.daily} w={120} h={36} />}
              icon="upgrade" />
            
          <StatCard
              label="Failures this week"
              value={failuresThisWeek}
              sub={failuresThisWeek === 0 ? 'all green' : 'review failed cycles'}
              tone={failuresThisWeek > 0 ? 'error' : 'neutral'}
              icon={failuresThisWeek > 0 ? 'error' : 'check_circle'} />
            
        </div>

        {/* Timeline */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 16 }} data-comment-anchor="b950c4f9d2-div-67-9">
          <div className="type-h2">Recent</div>
          <span className="chip outlined">{data.cycles.length} total</span>
          <div style={{ flex: 1 }} />
          <div style={{
              display: 'inline-flex', height: 36,
              background: 'var(--sc-low)', borderRadius: 999, padding: 3, gap: 2,
              color: 'var(--on-surface-variant)', fontSize: 12, fontWeight: 500
            }}>
            {['All', 'Updates', 'Failures'].map((t, i) =>
              <button key={t} style={{
                height: 30, padding: '0 14px', borderRadius: 999, border: 0, cursor: 'pointer',
                background: i === 0 ? 'var(--surface)' : 'transparent',
                color: i === 0 ? 'var(--on-surface)' : 'inherit',
                boxShadow: i === 0 ? 'var(--e1)' : 'none',
                font: 'inherit'
              }}>{t}</button>
              )}
          </div>
          <TimeWindowDropdown />
          <TriggerDropdown />
          <button className="btn icon small" title="Refresh"><span className="ms">refresh</span></button>
          <button className="btn icon small" title="Export"><span className="ms">file_download</span></button>
        </div>

        {/* Timeline rail */}
        <div style={{ position: 'relative', paddingLeft: 8 }}>
          {groups.map((g, gi) =>
            <div key={g.label} style={{ marginBottom: gi === groups.length - 1 ? 0 : 8 }}>
              {/* day header */}
              <div style={{
                display: 'flex', alignItems: 'center', gap: 12,
                margin: '12px 0 10px',
                paddingLeft: 0
              }}>
                <div style={{
                  width: 24, height: 24, borderRadius: 999,
                  background: 'var(--surface)',
                  border: '1.5px solid var(--outline-variant)',
                  display: 'grid', placeItems: 'center', flex: '0 0 24px',
                  color: 'var(--on-surface-variant)'
                }}>
                  <span className="ms" style={{ fontSize: 14 }}>today</span>
                </div>
                <div className="type-title" style={{ fontWeight: 600 }}>{g.label}</div>
                <div className="type-body-sm" style={{ color: 'var(--on-surface-muted)' }}>
                  {g.cycles.length} {g.cycles.length === 1 ? 'cycle' : 'cycles'}
                  {' · '}
                  {g.cycles.reduce((a, c) => a + c.updated, 0)} updated
                  {g.cycles.reduce((a, c) => a + c.failed + c.rolled_back, 0) > 0 &&
                  `, ${g.cycles.reduce((a, c) => a + c.failed + c.rolled_back, 0)} incident${g.cycles.reduce((a, c) => a + c.failed + c.rolled_back, 0) > 1 ? 's' : ''}`}
                </div>
                <div style={{ flex: 1, height: 1, background: 'var(--outline-variant)' }} />
              </div>

              {/* events */}
              <div style={{ position: 'relative', paddingLeft: 12 }}>
                {/* vertical line */}
                <div style={{
                  position: 'absolute',
                  left: 12 + 11.25, top: 0, bottom: 0, width: 1.5,
                  background: 'var(--outline-variant)'
                }} />
                {g.cycles.map((c, ci) =>
                <TimelineEvent key={c.id} c={c} H={H} now={data.summary.now}
                isExpanded={expanded === c.id}
                onToggle={() => setExpanded(expanded === c.id ? null : c.id)} />

                )}
              </div>
            </div>
            )}

          {visibleCount < data.cycles.length &&
            <div style={{ textAlign: 'center', padding: '16px 0' }}>
              <button className="btn outlined" onClick={() => setVisibleCount((c) => Math.min(c + 8, data.cycles.length))}>
                <span className="ms">expand_more</span>
                Load older ({data.cycles.length - visibleCount} remaining)
              </button>
            </div>
            }
        </div>
      </div>
      </div>
    </div>);

}

function HeroLastCycle({ c, outcome, now, H }) {
  // Stacked outcome bar
  const segments = [
  { k: 'updated', v: c.updated, color: 'var(--success)' },
  { k: 'rolled_back', v: c.rolled_back, color: 'var(--warning)' },
  { k: 'failed', v: c.failed, color: 'var(--error)' },
  { k: 'skipped', v: c.skipped, color: 'var(--neutral)' },
  { k: 'up_to_date', v: c.up_to_date, color: 'var(--outline-variant)' }];

  const total = c.scanned || 1;
  return (
    <div className="card" style={{
      padding: 24,
      background: 'linear-gradient(135deg, var(--primary-container) 0%, var(--sc-low) 70%)',
      color: 'var(--on-surface)',
      display: 'flex', flexDirection: 'column', gap: 16,
      position: 'relative', overflow: 'hidden'
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <span className="ms fill" style={{ fontSize: 20, color: 'var(--primary)' }}>radio_button_checked</span>
        <div className="type-overline" style={{ color: 'var(--on-surface)' }}>Last cycle</div>
        <div style={{ flex: 1 }} />
        <OutcomeChip outcome={
        outcome.kind === 'success' ? 'updated' :
        outcome.kind === 'warning' ? 'rolled_back' :
        outcome.kind === 'error' ? 'failed' :
        'up_to_date'
        } />
      </div>

      <div>
        <div style={{ display: 'flex', alignItems: 'baseline', gap: 12 }}>
          <div className="type-display" style={{ fontWeight: 500, fontSize: 44, letterSpacing: '-0.02em' }}>
            {H.fmtRelative(c.started_at, now)}
          </div>
          <TriggerChip trigger={c.trigger} />
        </div>
        <div className="type-body-sm" style={{ color: 'var(--on-surface-variant)', marginTop: 2 }}>
          {H.fmtAbs(c.started_at)} · ran for {H.fmtDuration(c.duration_sec)}
        </div>
      </div>

      {/* segmented outcome bar */}
      <div>
        <div style={{
          height: 10, borderRadius: 999, overflow: 'hidden',
          display: 'flex', background: 'var(--outline-variant)'
        }}>
          {segments.filter((s) => s.v > 0).map((s) =>
          <div key={s.k} title={`${s.v} ${s.k}`}
          style={{ width: `${s.v / total * 100}%`, background: s.color }} />
          )}
        </div>
        <div style={{ display: 'flex', gap: 14, marginTop: 10, flexWrap: 'wrap', fontSize: 12 }}>
          {segments.filter((s) => s.v > 0).map((s) =>
          <div key={s.k} style={{ display: 'flex', alignItems: 'center', gap: 6, color: 'var(--on-surface-variant)' }}>
              <span style={{ width: 8, height: 8, borderRadius: 2, background: s.color }} />
              <span style={{ fontWeight: 600, color: 'var(--on-surface)' }}>{s.v}</span>
              <span>{s.k.replace('_', '-')}</span>
            </div>
          )}
        </div>
      </div>
    </div>);

}

function TimelineStatTile({ label, value, sub, chart, tone, icon }) {
  return (
    <div className="card outlined" style={{
      padding: 18,
      display: 'flex', flexDirection: 'column', gap: 6,
      borderLeft: tone === 'error' ? '3px solid var(--error)' :
      tone === 'success' ? '3px solid var(--success)' :
      '1px solid var(--outline-variant)',
      boxShadow: tone === 'error' ? 'inset 1px 0 0 var(--error)' :
      tone === 'success' ? 'inset 1px 0 0 var(--success)' : null
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        {icon && <span className="ms" style={{ fontSize: 18, color: tone === 'error' ? 'var(--error)' : tone === 'success' ? 'var(--success)' : 'var(--on-surface-variant)' }}>{icon}</span>}
        <div className="type-overline">{label}</div>
        <div style={{ flex: 1 }} />
        {chart}
      </div>
      <div className="type-display type-num" style={{ fontSize: 36, fontWeight: 500, letterSpacing: '-0.02em' }}>{value}</div>
      <div className="type-body-sm" style={{ color: 'var(--on-surface-muted)' }}>{sub}</div>
    </div>);

}

function TimelineEvent({ c, H, now, isExpanded, onToggle }) {
  const outcome = c.failed ? 'failed' : c.rolled_back ? 'rolled_back' : c.updated ? 'updated' : 'up_to_date';
  const dotColor = outcome === 'failed' ? 'var(--error)' :
  outcome === 'rolled_back' ? 'var(--warning)' :
  outcome === 'updated' ? 'var(--success)' :
  'var(--outline)';
  const isQuiet = !c.updated && !c.failed && !c.rolled_back;

  return (
    <div style={{ position: 'relative', paddingLeft: 36, paddingBottom: 8 }}>
      <div style={{
        position: 'absolute', left: 6, top: 12,
        width: 10, height: 10, borderRadius: 999,
        background: isQuiet ? 'var(--outline-variant)' : dotColor,
        // Single bg-colored ring punches a hole in the timeline rule —
        // no second colored ring around it, which used to read as a
        // selected radio button.
        boxShadow: `0 0 0 3px var(--bg)`
      }} />
      <div onClick={onToggle} className="card"
      style={{
        background: isExpanded ? 'var(--sc)' : 'var(--sc-low)',
        padding: isQuiet && !isExpanded ? '10px 16px' : '14px 18px',
        cursor: 'pointer',
        boxShadow: isExpanded ? 'var(--e2)' : 'none',
        transition: 'background .12s',
        opacity: isQuiet ? 0.85 : 1
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
          <div style={{ width: 64, flex: '0 0 64px' }}>
            <div className="type-mono type-num" style={{ fontSize: 14, fontWeight: 600 }}>{H.fmtTime(c.started_at)}</div>
            <div className="type-body-sm" style={{ color: 'var(--on-surface-muted)', fontSize: 11 }}>{H.fmtDuration(c.duration_sec)}</div>
          </div>
          <TriggerChip trigger={c.trigger} />
          <div style={{ flex: 1, display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}>
            {isQuiet ?
            <div className="type-body" style={{ color: 'var(--on-surface-variant)' }}>
                Scanned {c.scanned} containers — all up to date
              </div> :

            <div className="type-body" style={{ color: 'var(--on-surface)' }}>
                {c.updated > 0 && <><b>{c.updated}</b> updated</>}
                {c.updated > 0 && (c.rolled_back > 0 || c.failed > 0) && ', '}
                {c.rolled_back > 0 && <><b style={{ color: 'var(--warning)' }}>{c.rolled_back}</b> rolled back</>}
                {c.rolled_back > 0 && c.failed > 0 && ', '}
                {c.failed > 0 && <><b style={{ color: 'var(--error)' }}>{c.failed}</b> failed</>}
                <span style={{ color: 'var(--on-surface-muted)' }}> · {c.up_to_date} unchanged</span>
              </div>
            }
          </div>
          {/* preview chips for changed containers */}
          {!isQuiet && !isExpanded &&
          <div style={{ display: 'flex', gap: 4 }}>
              {c.containers.filter((cc) => cc.outcome !== 'up_to_date' && cc.outcome !== 'skipped').slice(0, 4).map((cc, i) =>
            <span key={i} className="chip" style={{
              background: 'var(--surface-bright)', color: 'var(--on-surface)',
              height: 22, padding: '0 8px', fontSize: 11
            }}>
                  <span style={{
                width: 6, height: 6, borderRadius: 999,
                background: cc.outcome === 'failed' ? 'var(--error)' :
                cc.outcome === 'rolled_back' ? 'var(--warning)' :
                'var(--success)'
              }} />
                  {cc.name}
                </span>
            )}
            </div>
          }
          <button className="btn icon small" style={{
            transform: isExpanded ? 'rotate(180deg)' : 'none',
            transition: 'transform .15s'
          }}>
            <span className="ms">expand_more</span>
          </button>
        </div>

        {isExpanded &&
        <div style={{ marginTop: 14, marginLeft: 78, borderTop: '1px solid var(--outline-variant)', paddingTop: 12 }}>
            <div style={{ display: 'flex', gap: 18, marginBottom: 10, color: 'var(--on-surface-variant)', fontSize: 12 }}>
              <span><b style={{ color: 'var(--on-surface)' }}>#{c.id}</b> · {H.fmtAbs(c.started_at)}</span>
              <span>scanned {c.scanned}</span>
              <span>duration {H.fmtDuration(c.duration_sec)}</span>
              <span>completed {H.fmtTime(c.completed_at)}</span>
            </div>
            <div className="card outlined" style={{ background: 'var(--surface)' }}>
              {c.containers.filter((cc) => cc.outcome !== 'up_to_date').slice(0, 8).map((cc, i) =>
            <ContainerOutcomeRow key={cc.name + i} c={cc} dense />
            )}
              {c.containers.filter((cc) => cc.outcome === 'up_to_date').length > 0 &&
            <div style={{ padding: '8px 16px', color: 'var(--on-surface-muted)', fontSize: 12, fontStyle: 'italic' }}>
                  + {c.containers.filter((cc) => cc.outcome === 'up_to_date').length} containers up to date
                </div>
            }
            </div>
          </div>
        }
      </div>
    </div>);

}

function StatCard({ label, value, sub, badge, chart, tone = 'neutral', icon }) {
  return (
    <div className="card" style={{
      padding: 20,
      background: tone === 'error' ? 'var(--error-container)' : 'var(--sc-low)',
      color: tone === 'error' ? 'var(--on-error-container)' : 'var(--on-surface)',
      display: 'flex', flexDirection: 'column', gap: 8, minHeight: 132,
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <span className="ms" style={{ fontSize: 18, color: 'inherit', opacity: .85 }}>{icon}</span>
        <div className="type-overline" style={{ color: 'inherit', opacity: .8 }}>{label}</div>
        <div style={{ flex: 1 }} />
        {badge}
      </div>
      <div style={{ display: 'flex', alignItems: 'baseline', gap: 12, justifyContent: 'space-between' }}>
        <div className="type-display type-num" style={{ fontSize: 40, fontWeight: 500, letterSpacing: '-0.02em', color: 'inherit' }}>{value}</div>
        {chart}
      </div>
      <div className="type-body-sm" style={{ color: tone === 'error' ? 'inherit' : 'var(--on-surface-muted)', opacity: tone === 'error' ? .85 : 1 }}>{sub}</div>
    </div>
  );
}

window.LayoutTimeline = LayoutTimeline;
window.StatCard = StatCard;

function TimeWindowDropdown() {
  return (
    <FilterDropdown
      icon="event"
      defaultValue="Last 7 days"
      options={['Last 24 hours', 'Last 7 days', 'Last 30 days', 'Last 90 days', 'All time']}
    />
  );
}

function TriggerDropdown() {
  return (
    <FilterDropdown
      icon="bolt"
      defaultValue="All triggers"
      options={['All triggers', 'Scheduled', 'Manual', 'HTTP API']}
    />
  );
}

function FilterDropdown({ icon, defaultValue, options }) {
  const [open, setOpen] = React.useState(false);
  const [value, setValue] = React.useState(defaultValue);
  const ref = React.useRef(null);
  React.useEffect(() => {
    if (!open) return;
    const onDoc = (e) => { if (ref.current && !ref.current.contains(e.target)) setOpen(false); };
    document.addEventListener('mousedown', onDoc);
    return () => document.removeEventListener('mousedown', onDoc);
  }, [open]);
  return (
    <div ref={ref} style={{ position: 'relative' }}>
      <button
        onClick={() => setOpen(o => !o)}
        style={{
          display: 'inline-flex', alignItems: 'center', gap: 6,
          height: 36, padding: '0 10px 0 12px',
          borderRadius: 999,
          background: open ? 'var(--sc)' : 'var(--sc-low)',
          color: 'var(--on-surface)',
          border: 0, font: 'inherit', fontSize: 12, fontWeight: 500,
          cursor: 'pointer', transition: 'background .12s',
        }}>
        <span className="ms" style={{ fontSize: 16, color: 'var(--on-surface-variant)' }}>{icon}</span>
        {value}
        <span className="ms" style={{ fontSize: 18, color: 'var(--on-surface-variant)', transform: open ? 'rotate(180deg)' : 'none', transition: 'transform .15s' }}>arrow_drop_down</span>
      </button>
      {open && (
        <div className="card" style={{
          position: 'absolute', top: 'calc(100% + 6px)', right: 0,
          minWidth: 180, padding: 4, zIndex: 10,
          background: 'var(--surface)',
          boxShadow: 'var(--e2)',
          border: '1px solid var(--outline-variant)',
        }}>
          {options.map((opt) => (
            <button key={opt}
              onClick={() => { setValue(opt); setOpen(false); }}
              style={{
                display: 'flex', alignItems: 'center', gap: 8,
                width: '100%', height: 36, padding: '0 12px',
                borderRadius: 6,
                background: opt === value ? 'var(--primary-soft)' : 'transparent',
                color: 'var(--on-surface)',
                border: 0, cursor: 'pointer', font: 'inherit', fontSize: 13,
                textAlign: 'left',
              }}
              onMouseEnter={(e) => { if (opt !== value) e.currentTarget.style.background = 'var(--sc-low)'; }}
              onMouseLeave={(e) => { if (opt !== value) e.currentTarget.style.background = 'transparent'; }}
            >
              <span className="ms" style={{ fontSize: 16, color: opt === value ? 'var(--primary)' : 'transparent' }}>check</span>
              {opt}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}