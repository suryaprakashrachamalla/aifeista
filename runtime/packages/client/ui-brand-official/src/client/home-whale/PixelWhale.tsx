/** Home frame layer: continuous ambient starlight + choreographed magical AI Carnival animation on blank-home clicks & mount. */
import { useCallback, useEffect, useRef, type MouseEvent, type ReactNode } from 'react'
import type { HeroFrameOwnerProps } from '@deepseek-ai/dsh-client-ui-conversation/client'
import { CARNIVAL_WHEEL_ARTWORK } from './artwork.ts'
import { attachPixelWhale, type PixelWhaleHandle } from './renderer.ts'
import css from './PixelWhale.module.css'

/** Interactive descendants whose clicks belong to controls, not to blank home space. */
const CONTROLS = 'button, a, input, textarea, select, label, [contenteditable], [role="button"], [role="dialog"], [role="menu"], [role="listbox"], [data-composer-card]'

export type PixelWhaleProps = HeroFrameOwnerProps & { children: ReactNode }

function isBlankClick(event: MouseEvent<HTMLDivElement>): boolean {
  const target = event.target as Element
  const frame = event.currentTarget
  const selection = window.getSelection()
  return event.button === 0
    && frame.contains(target)
    && target.closest(CONTROLS) === null
    && !(selection !== null && !selection.isCollapsed && frame.contains(selection.anchorNode))
}

/**
 * Wrap the Conversation scroll body; while the home is shown, render the 60fps
 * magical shooting-star, vortex-swirl, zoom-in/zoom-out Circus Wheel & pixels/text canvas.
 */
export function PixelWhale({ hero, children }: PixelWhaleProps) {
  const wheel = useRef<PixelWhaleHandle | undefined>(undefined)

  useEffect(() => {
    const onMagic = (): void => { wheel.current?.play() }
    window.addEventListener('carnival:magic', onMagic)
    if (!hero) return () => { window.removeEventListener('carnival:magic', onMagic) }
    const timer = window.setTimeout(() => { wheel.current?.play() }, 500)
    const interval = window.setInterval(() => {
      if (!document.hidden) wheel.current?.play()
    }, 26000)
    return () => {
      window.clearTimeout(timer)
      window.clearInterval(interval)
      window.removeEventListener('carnival:magic', onMagic)
    }
  }, [hero])

  const canvasRef = useCallback((canvas: HTMLCanvasElement | null) => {
    wheel.current?.dispose()
    wheel.current = canvas === null ? undefined : attachPixelWhale(canvas, CARNIVAL_WHEEL_ARTWORK)
  }, [])

  const onClick = (event: MouseEvent<HTMLDivElement>): void => {
    if (wheel.current !== undefined && isBlankClick(event)) wheel.current.play()
  }

  return (
    <div className={css.layer} onClick={onClick}>
      {hero && <canvas ref={canvasRef} className={css.canvas} data-home-carnival-wheel="" aria-hidden="true" />}
      {children}
    </div>
  )
}
