// The composer remains in ConversationRoot so switching out of the blank-draft
// phase does not remount its textarea.

import { useState } from 'react'
import type { ReactNode, RefObject } from 'react'
import {
  FISH_LOGO_VIEWBOX, IconChevronDownOutlineRegular, IconFolderCloseRegular, IconFolderOpenRegular,
} from '@carnival/dsh-client-ui-primitives'
import { workspaceTitleOf } from '@carnival/dsh-util-workspace-path'
import type { ConversationContentProps } from '../contract/slots.ts'
import css from './HeroShell.module.css'

/** The owner's locale seat type, passed to hero chrome as a plain prop. */
type HeroTranslate = ConversationContentProps['t']

const WHEEL_ANGLES = [0, 45, 90, 135, 180, 225, 270, 315] as const

/**
 * Basename label for the studio chip (the shared derivation);
 * separator-only paths echo the raw cwd.
 * @param cwd - studio directory path (non-empty).
 * @returns chip label.
 */
export function workspaceLabel(cwd: string): string {
  const base = workspaceTitleOf(cwd)
  return base !== '' ? base : cwd
}

/**
 * The studio chip (folder + label + chevron), always interactive before
 * the first message.
 */
export function WorkspaceChip({ buttonRef, label, menuOpen = false, onClick, t }: {
  buttonRef?: RefObject<HTMLButtonElement>
  label?: string | undefined
  menuOpen?: boolean
  onClick?: () => void
  t: HeroTranslate
}) {
  return (
    <button
      ref={buttonRef}
      type="button"
      className={css.workspace}
      aria-label={t('hero.chooseWorkspace')}
      aria-haspopup="menu"
      aria-expanded={menuOpen}
      onClick={onClick}
    >
      {label === undefined
        ? <IconFolderCloseRegular className={css.folder} size={16} />
        : <IconFolderOpenRegular className={css.folder} size={16} />}
      <span className={css.workspaceLabel}>{label ?? t('hero.chooseWorkspace')}</span>
      <IconChevronDownOutlineRegular className={css.chevron} size={12} />
    </button>
  )
}

/** Hero chrome props. */
export interface HeroShellProps {
  t: HeroTranslate
  renderSlot: ConversationContentProps['renderSlot']
  headline?: string | undefined
  children?: ReactNode
}

/**
 * The hero AI Carnival Circus Ferris Wheel (44px wide):
 * gracefully rotates at rest (26s calm turn) and accelerates joyfully on hover (3.4s spin)
 * with warm marquee glow while the grounded A-frame support stays stationary.
 */
function HeroWheel({ hovering }: { hovering: boolean }) {
  const reducedMotion = typeof window !== 'undefined'
    && window.matchMedia('(prefers-reduced-motion: reduce)').matches
  return (
    <svg
      className={css.fish}
      width={44}
      height={(44 * FISH_LOGO_VIEWBOX.height) / FISH_LOGO_VIEWBOX.width}
      viewBox={`0 0 ${FISH_LOGO_VIEWBOX.width} ${FISH_LOGO_VIEWBOX.height}`}
      fill="none"
      aria-hidden="true"
    >
      {/* Static grounded A-frame support tower */}
      <path
        d="M12 11.4L6.6 21.5M12 11.4L17.4 21.5M5 21.5H19"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      {/* Rotating Circus Ferris Wheel assembly */}
      <g>
        {!reducedMotion && (
          <animateTransform
            key={hovering ? 'spin-fast' : 'spin-calm'}
            attributeName="transform"
            type="rotate"
            from="0 12 10.4"
            to="360 12 10.4"
            dur={hovering ? '3.4s' : '26s'}
            repeatCount="indefinite"
          />
        )}
        <circle cx="12" cy="10.4" r="7.8" stroke="currentColor" strokeWidth="1.45" />
        <circle cx="12" cy="10.4" r="4.6" stroke="currentColor" strokeWidth="0.9" opacity="0.65" />
        {WHEEL_ANGLES.map((deg) => {
          const rad = (deg * Math.PI) / 180
          const gx = Number((12 + 7.8 * Math.cos(rad)).toFixed(2))
          const gy = Number((10.4 + 7.8 * Math.sin(rad)).toFixed(2))
          return (
            <g key={deg}>
              <line x1="12" y1="10.4" x2={gx} y2={gy} stroke="currentColor" strokeWidth="1" />
              <circle cx={gx} cy={gy} r="1.15" fill="currentColor" />
            </g>
          )
        })}
      </g>
      {/* Center axle hub */}
      <circle cx="12" cy="10.4" r="1.7" fill="currentColor" />
    </svg>
  )
}

/**
 * Render the AI Carnival hero chrome with editorial typography & animated Circus Wheel.
 */
export function HeroShell({ t, renderSlot, headline, children }: HeroShellProps) {
  const [hovering, setHovering] = useState(false)
  return (
    <div className={css.root}>
      <div className={css.stack}>
        <div className={css.headline}>
          <span
            className={css.fishHitbox}
            onPointerEnter={(event) => {
              if (event.pointerType !== 'touch'
                && window.matchMedia('(hover: hover) and (prefers-reduced-motion: no-preference)').matches) {
                setHovering(true)
              }
            }}
            onPointerLeave={() => { setHovering(false) }}
            onClick={() => { window.dispatchEvent(new Event('carnival:magic')) }}
          >
            {renderSlot('conversation.hero.brand.mark', { size: 44, className: css.fish }, {
              fallback: <HeroWheel hovering={hovering} />,
            })}
          </span>
          <span className={css.titleGroup}>
            <span className={css.fancyHeadline}>{headline ?? t('hero.headline')}</span>
          </span>
        </div>
        <div className={css.body}>
          {/* The composer remains mounted outside this component. */}
        </div>
      </div>
      {children}
    </div>
  )
}
