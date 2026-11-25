/** General Settings control for Carnival Glass Appearance and Runtime engines. */
import { useEffect, useState } from 'react'
import { Switch } from '@carnival/dsh-client-ui-primitives'
import css from './DeveloperToolsRow.module.css'

const BRIDGE = 'http://127.0.0.1:32123'

const FINISHES = [
  { id: 'obsidian', label: 'Obsidian' },
  { id: 'jet', label: 'Jet Black' },
  { id: 'warm-smoke', label: 'Warm Smoke' },
] as const

/**
 * Render Carnival's monochrome Glass Opacity, Glass Finish, and Runtime switches inside Settings -> General.
 */
export function GlassAndRuntimeRow() {
  const [opacity, setOpacity] = useState<number>(() => {
    const saved = Number.parseFloat(localStorage.getItem('carnival.glassOpacity') || '0.80')
    return Number.isFinite(saved) ? Math.min(0.96, Math.max(0.08, saved)) : 0.80
  })
  const [tint, setTint] = useState<string>(() => localStorage.getItem('carnival.glassTint') || 'obsidian')
  const [companionOverlay, setCompanionOverlay] = useState(false)
  const [cache, setCache] = useState(true)
  const [skills, setSkills] = useState(true)
  const [forge, setForge] = useState(true)
  const [localTriage, setLocalTriage] = useState(true)
  const [cordisState, setCordisState] = useState<string>('stopped')

  useEffect(() => {
    void Promise.all([
      fetch(`${BRIDGE}/telemetry`).then(r => r.json()).catch(() => null),
      fetch(`${BRIDGE}/overlay/state`).then(r => r.json()).catch(() => null),
    ]).then(([tel, ov]) => {
      if (tel && typeof tel === 'object') {
        const opts = (tel as { optimizations?: Record<string, boolean> }).optimizations
        if (opts !== undefined) {
          setCache(Boolean(opts.prefix_cache_lock))
          setSkills(Boolean(opts.skill_tree_pruning))
          setForge(Boolean(opts.ptc_batching))
          setLocalTriage(Boolean(opts.local_triage))
        }
        const cs = (tel as { cordis_state?: string }).cordis_state
        if (cs) setCordisState(cs)
      }
      if (ov && typeof ov === 'object') {
        const o = ov as { visible?: boolean; glass_opacity?: number; glass_tint?: string }
        setCompanionOverlay(Boolean(o.visible))
        if (typeof o.glass_opacity === 'number') setOpacity(o.glass_opacity)
        if (typeof o.glass_tint === 'string' && FINISHES.some(f => f.id === o.glass_tint)) {
          setTint(o.glass_tint)
        }
      }
    })
  }, [])

  const applyGlass = (nextOpacity: number, nextTint: string): void => {
    setOpacity(nextOpacity)
    setTint(nextTint)
    localStorage.setItem('carnival.glassOpacity', nextOpacity.toFixed(2))
    localStorage.setItem('carnival.glassTint', nextTint)
    window.dispatchEvent(new CustomEvent('carnival:set-glass', {
      detail: { opacity: nextOpacity, tint: nextTint },
    }))
  }

  const pushOpt = (patch: Partial<{ cache: boolean; skills: boolean; forge: boolean; localTriage: boolean }>): void => {
    const nextCache = patch.cache ?? cache
    const nextSkills = patch.skills ?? skills
    const nextForge = patch.forge ?? forge
    const nextLocal = patch.localTriage ?? localTriage
    setCache(nextCache)
    setSkills(nextSkills)
    setForge(nextForge)
    setLocalTriage(nextLocal)
    void fetch(`${BRIDGE}/optimizations`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        prefix_cache_lock: nextCache,
        skill_tree_pruning: nextSkills,
        ptc_batching: nextForge,
        local_triage: nextLocal,
      }),
    }).catch(() => {})
  }

  return (
    <div>
      <div className={css.row}>
        <div>
          <div className={css.title}>Glass Opacity</div>
          <div className={css.description}>Adjust macOS smoked glass transparency ({Math.round(opacity * 100)}%)</div>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <input
            type="range"
            min={8}
            max={96}
            value={Math.round(opacity * 100)}
            aria-label="Glass Opacity"
            onChange={(e) => { applyGlass(Number(e.target.value) / 100, tint) }}
            style={{ width: 136, accentColor: '#ede6d6', cursor: 'pointer' }}
          />
          <span style={{ minWidth: 36, textAlign: 'right', fontSize: 12, color: 'var(--dsw-alias-label-secondary)' }}>
            {Math.round(opacity * 100)}%
          </span>
          {(Math.abs(opacity - 0.80) >= 0.015 || tint !== 'obsidian') && (
            <button
              type="button"
              onClick={() => { applyGlass(0.80, 'obsidian') }}
              style={{
                padding: '3px 8px',
                borderRadius: 6,
                border: '1px solid var(--dsw-alias-border-l2)',
                background: 'rgba(255,255,255,0.05)',
                color: '#f5f2eb',
                fontSize: 11,
                cursor: 'pointer',
              }}
            >
              Reset
            </button>
          )}
        </div>
      </div>

      <div className={css.row}>
        <div>
          <div className={css.title}>Glass Finish</div>
          <div className={css.description}>Choose neutral obsidian, pure jet black, or warm smoke tint</div>
        </div>
        <div style={{ display: 'inline-flex', gap: 4, padding: 3, borderRadius: 10, border: '1px solid var(--dsw-alias-border-l2)', background: 'rgba(255,255,255,0.04)' }}>
          {FINISHES.map(f => (
            <button
              key={f.id}
              type="button"
              onClick={() => { applyGlass(opacity, f.id) }}
              style={{
                padding: '4px 10px',
                borderRadius: 7,
                border: '1px solid',
                borderColor: tint === f.id ? 'rgba(245, 242, 235, 0.24)' : 'transparent',
                background: tint === f.id ? 'rgba(245, 242, 235, 0.14)' : 'transparent',
                color: tint === f.id ? '#f5f2eb' : 'var(--dsw-alias-label-secondary)',
                fontSize: 12,
                fontWeight: 500,
                cursor: 'pointer',
              }}
            >
              {f.label}
            </button>
          ))}
        </div>
      </div>

      <div className={css.row}>
        <div>
          <div className={css.title}>Voice Companion Overlay</div>
          <div className={css.description}>Show transparent pointer and spatial callout HUD ({cordisState})</div>
        </div>
        <Switch
          checked={companionOverlay}
          label="Voice Companion Overlay"
          onChange={(next) => {
            setCompanionOverlay(next)
            void fetch(`${BRIDGE}/window/overlay`, {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({ action: next ? 'show' : 'hide' }),
            }).catch(() => {})
          }}
        />
      </div>

      <div className={css.row}>
        <div>
          <div className={css.title}>Prefix-Cache Lock</div>
          <div className={css.description}>Keep volatile prompt prefixes pinned for sub-100ms turns</div>
        </div>
        <Switch checked={cache} label="Prefix-Cache Lock" onChange={(next) => { pushOpt({ cache: next }) }} />
      </div>

      <div className={css.row}>
        <div>
          <div className={css.title}>Skill Tree Lazy Pruning</div>
          <div className={css.description}>Load only turn-relevant capabilities into model context</div>
        </div>
        <Switch checked={skills} label="Skill Tree Lazy Pruning" onChange={(next) => { pushOpt({ skills: next }) }} />
      </div>

      <div className={css.row}>
        <div>
          <div className={css.title}>Codeforge Batching</div>
          <div className={css.description}>Batch independent programmatic tool calls cleanly</div>
        </div>
        <Switch checked={forge} label="Codeforge Batching" onChange={(next) => { pushOpt({ forge: next }) }} />
      </div>

      <div className={css.row}>
        <div>
          <div className={css.title}>Local Triage</div>
          <div className={css.description}>Route lightweight lookups locally for sub-second responses</div>
        </div>
        <Switch checked={localTriage} label="Local Triage" onChange={(next) => { pushOpt({ localTriage: next }) }} />
      </div>
    </div>
  )
}
