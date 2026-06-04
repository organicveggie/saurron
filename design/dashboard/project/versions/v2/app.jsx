// Saurron Dashboard — main app. Composes the design canvas + 3 artboards
// + tweaks panel for accent / theme persistence.

const { useState, useEffect } = React;

const ACCENTS = [
  { id: 'saurron', label: 'Saurron', color: 'oklch(0.72 0.15 55)' },
  { id: 'ember',   label: 'Ember',   color: 'oklch(0.65 0.18 35)' },
  { id: 'teal',    label: 'Teal',    color: 'oklch(0.68 0.11 200)' },
  { id: 'indigo',  label: 'Indigo',  color: 'oklch(0.62 0.13 285)' },
];

// Custom accent swatch picker — TweakColor only handles raw colors, not
// named tokens, so we render a small swatch row by hand using the panel's
// row chrome.
function AccentPicker({ value, onChange }) {
  return (
    <div style={{ padding: '6px 14px 10px' }}>
      <div style={{
        fontSize: 11, fontWeight: 600, letterSpacing: '0.06em', textTransform: 'uppercase',
        color: 'rgba(255,255,255,0.55)', marginBottom: 8,
      }}>Accent</div>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 8 }}>
        {ACCENTS.map((a) => {
          const on = value === a.id;
          return (
            <button key={a.id}
              onClick={() => onChange(a.id)}
              title={a.label}
              style={{
                display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 6,
                padding: '6px 4px',
                background: 'transparent', border: 0, cursor: 'pointer',
                font: 'inherit',
              }}>
              <div style={{
                width: '100%', height: 32, borderRadius: 8,
                background: a.color,
                boxShadow: on
                  ? `0 0 0 2px rgba(255,255,255,0.95), 0 0 0 4px ${a.color}`
                  : 'inset 0 0 0 1px rgba(255,255,255,0.1)',
                transition: 'box-shadow .15s',
              }} />
              <span style={{
                fontSize: 11,
                color: on ? 'rgba(255,255,255,0.95)' : 'rgba(255,255,255,0.55)',
                fontWeight: on ? 600 : 500,
              }}>{a.label}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

function App() {
  const [tweaks, setTweak] = useTweaks(window.__SAURRON_TWEAK_DEFAULTS);

  // Mirror tweak state into document.body data attrs so all .saurron-app
  // children pick up the active palette / theme via CSS variables.
  useEffect(() => {
    document.body.setAttribute('data-theme',  tweaks.theme);
    document.body.setAttribute('data-accent', tweaks.accent);
  }, [tweaks.theme, tweaks.accent]);

  // The TopAppBar's theme toggle posts __edit_mode_set_keys directly; this
  // listener keeps our local state in sync when that happens.
  useEffect(() => {
    const onMsg = (e) => {
      if (e.data?.type === '__edit_mode_set_keys' && e.data.edits?.theme) {
        setTweak('theme', e.data.edits.theme);
      }
    };
    window.addEventListener('message', onMsg);
    return () => window.removeEventListener('message', onMsg);
  }, []);

  const data = window.SAURRON_DATA;

  // Each artboard wraps its layout in .saurron-app so the MD3 token scope
  // is restricted to the dashboard content and doesn't bleed into the
  // design-canvas chrome.
  const wrap = (Layout) => (
    <div className="saurron-app" data-theme={tweaks.theme} data-accent={tweaks.accent}
      style={{ width: '100%', height: '100%', display: 'flex', overflow: 'hidden' }}>
      <Layout data={data} />
    </div>
  );

  return (
    <>
      <DesignCanvas>
        <DCSection
          id="dashboards"
          title="Saurron dashboards"
          subtitle="Material Design 3 · activity feed layout · dark theme by default · tweak palette below"
        >
          <DCArtboard id="timeline" label="Activity feed"
            width={1360} height={920}>
            {wrap(LayoutTimeline)}
          </DCArtboard>
        </DCSection>
      </DesignCanvas>

      <TweaksPanel title="Tweaks">
        <TweakSection label="Palette">
          <AccentPicker
            value={tweaks.accent}
            onChange={(v) => setTweak('accent', v)}
          />
          <TweakRadio
            label="Theme"
            value={tweaks.theme}
            onChange={(v) => setTweak('theme', v)}
            options={[
              { value: 'dark',  label: 'Dark' },
              { value: 'light', label: 'Light' },
            ]}
          />
        </TweakSection>
        <TweakSection label="About">
          <div style={{ fontSize: 12, color: 'rgba(255,255,255,0.6)', lineHeight: 1.5, padding: '4px 14px 0' }}>
            An activity-feed take on the Material Design 3 dashboard. The full
            palette is derived from a single hue via{' '}
            <code style={{ background: 'rgba(255,255,255,0.08)', padding: '1px 5px', borderRadius: 3, fontSize: 11 }}>oklch()</code>,
            so role colors stay coherent across every accent.
          </div>
          <div style={{ fontSize: 11, color: 'rgba(255,255,255,0.45)', padding: '6px 14px 8px', lineHeight: 1.5 }}>
            Click the artboard's expand button (top-right of the card) to view
            it fullscreen. Row-expand and theme toggle all work.
          </div>
        </TweakSection>
      </TweaksPanel>
    </>
  );
}

const root = ReactDOM.createRoot(document.getElementById('root'));
root.render(<App />);
