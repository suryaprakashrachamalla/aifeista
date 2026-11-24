/**
 * Silky 60fps Monochrome Silk Starlight & Smooth Continuous Ribbon Engine for Carnival.
 * Eliminates harsh polygon cutouts, blocky squares, and beaded stroke steps in favor of
 * sub-pixel drifting cream/white stardust and single-path tapered Bezier light ribbons.
 */
import type { CarnivalWheelArtwork } from './artwork.ts'

export interface PixelWhaleHandle {
  play: () => void
  dispose: () => void
}

const NO_WHEEL: PixelWhaleHandle = { play: () => {}, dispose: () => {} }

interface AmbientMote {
  nx: number
  ny: number
  radius: number
  phase: number
  twinkleSpeed: number
  vx: number
  vy: number
  warmth: number
  halo: boolean
}

function clamp01(v: number): number {
  return v < 0 ? 0 : v > 1 ? 1 : v
}

function smoothstep(edge0: number, edge1: number, x: number): number {
  const t = clamp01((x - edge0) / (edge1 - edge0))
  return t * t * (3 - 2 * t)
}

function cubicBezierPoint(
  t: number,
  p0: [number, number],
  p1: [number, number],
  p2: [number, number],
  p3: [number, number],
): [number, number] {
  const u = 1 - t
  const tt = t * t
  const uu = u * u
  const uuu = uu * u
  const ttt = tt * t
  return [
    uuu * p0[0] + 3 * uu * t * p1[0] + 3 * u * tt * p2[0] + ttt * p3[0],
    uuu * p0[1] + 3 * uu * t * p1[1] + 3 * u * tt * p2[1] + ttt * p3[1],
  ]
}

function cubicBezierTangent(
  t: number,
  p0: [number, number],
  p1: [number, number],
  p2: [number, number],
  p3: [number, number],
): [number, number] {
  const u = 1 - t
  const dx =
    3 * u * u * (p1[0] - p0[0]) +
    6 * u * t * (p2[0] - p1[0]) +
    3 * t * t * (p3[0] - p2[0])
  const dy =
    3 * u * u * (p1[1] - p0[1]) +
    6 * u * t * (p2[1] - p1[1]) +
    3 * t * t * (p3[1] - p2[1])
  const len = Math.hypot(dx, dy) || 1
  return [dx / len, dy / len]
}

function drawRadialBloom(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  radius: number,
  innerRgba: string,
  midRgba: string,
): void {
  if (radius <= 1) return
  const grad = ctx.createRadialGradient(x, y, 0, x, y, radius)
  grad.addColorStop(0, innerRgba)
  grad.addColorStop(0.42, midRgba)
  grad.addColorStop(1, 'transparent')
  ctx.fillStyle = grad
  ctx.beginPath()
  ctx.arc(x, y, radius, 0, Math.PI * 2)
  ctx.fill()
}

/**
 * Draw a single continuous tapered polygon ribbon along a cubic Bezier spline,
 * filled with one unified linear gradient so there are zero beaded segment seams.
 */
function drawSilkRibbon(
  ctx: CanvasRenderingContext2D,
  t: number,
  fade: number,
  p0: [number, number],
  p1: [number, number],
  p2: [number, number],
  p3: [number, number],
): void {
  if (fade <= 0.01 || t <= 0.02) return
  const tailT = Math.max(0, t - 0.36)
  const samples = 72
  const leftSide: [number, number][] = []
  const rightSide: [number, number][] = []

  for (let i = 0; i <= samples; i++) {
    const frac = i / samples
    const s = tailT + (t - tailT) * frac
    const [px, py] = cubicBezierPoint(s, p0, p1, p2, p3)
    const [tx, ty] = cubicBezierTangent(s, p0, p1, p2, p3)
    const nx = -ty
    const ny = tx
    // Smooth teardrop taper: 0px at tail -> 2.1px near head -> 0.35px tip
    const halfW = Math.sin(Math.pow(frac, 0.72) * Math.PI * 0.92) * 2.1 * (0.35 + 0.65 * fade)
    leftSide.push([px + nx * halfW, py + ny * halfW])
    rightSide.push([px - nx * halfW, py - ny * halfW])
  }

  const [sx, sy] = cubicBezierPoint(tailT, p0, p1, p2, p3)
  const [hx, hy] = cubicBezierPoint(t, p0, p1, p2, p3)

  ctx.save()
  // Soft outer silk trail halo + crisp continuous core polygon
  const grad = ctx.createLinearGradient(sx, sy, hx, hy)
  grad.addColorStop(0, 'rgba(237, 230, 214, 0)')
  grad.addColorStop(0.55, `rgba(237, 230, 214, ${(0.22 * fade).toFixed(3)})`)
  grad.addColorStop(0.92, `rgba(247, 244, 236, ${(0.62 * fade).toFixed(3)})`)
  grad.addColorStop(1, `rgba(255, 255, 255, ${(0.78 * fade).toFixed(3)})`)

  ctx.fillStyle = grad
  ctx.beginPath()
  for (let i = 0; i < leftSide.length; i++) {
    const [lx, ly] = leftSide[i] as [number, number]
    if (i === 0) ctx.moveTo(lx, ly)
    else ctx.lineTo(lx, ly)
  }
  for (let i = rightSide.length - 1; i >= 0; i--) {
    const [rx, ry] = rightSide[i] as [number, number]
    ctx.lineTo(rx, ry)
  }
  ctx.closePath()
  ctx.fill()

  // Delicate pearl core light at the leading point (no harsh 4-point ClipArt star)
  drawRadialBloom(
    ctx,
    hx,
    hy,
    24,
    `rgba(255, 255, 255, ${(0.42 * fade).toFixed(3)})`,
    `rgba(237, 230, 214, ${(0.12 * fade).toFixed(3)})`,
  )
  ctx.fillStyle = `rgba(255, 255, 255, ${(0.90 * fade).toFixed(3)})`
  ctx.beginPath()
  ctx.arc(hx, hy, 2.0, 0, Math.PI * 2)
  ctx.fill()
  ctx.restore()
}

export function attachPixelWhale(
  canvas: HTMLCanvasElement,
  options: CarnivalWheelArtwork,
  _unusedAssetUrl?: string,
): PixelWhaleHandle {
  const context = canvas.getContext('2d')
  if (context === null) return NO_WHEEL

  const ctx = context
  const media = matchMedia('(prefers-reduced-motion: reduce)')
  let disposed = false
  let visible = true
  let rafId = 0
  let layoutRaf = 0
  let width = 0
  let height = 0
  let burstStarted: number | undefined

  let seed = 0x9e3779b9
  const rnd = () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0
    return seed / 0x1_0000_0000
  }

  // 38 delicate, light warm-white/cream ambient stardust motes
  const motes: AmbientMote[] = Array.from({ length: 38 }, (_, i) => ({
    nx: rnd(),
    ny: rnd(),
    radius: 0.55 + rnd() * 0.95,
    phase: rnd() * Math.PI * 2,
    twinkleSpeed: 0.35 + rnd() * 0.75,
    vx: (rnd() - 0.5) * 0.0018,
    vy: -0.0008 - rnd() * 0.0014,
    warmth: rnd(),
    halo: i < 7,
  }))

  const moving = () => !media.matches
  const drawable = () => !disposed && !document.hidden && visible && width > 0 && height > 0

  function drawStardustLayer(nowSec: number): void {
    ctx.save()
    for (const m of motes) {
      const driftX = ((m.nx + nowSec * m.vx + Math.sin(nowSec * 0.18 + m.phase) * 0.008) % 1 + 1) % 1
      const driftY = ((m.ny + nowSec * m.vy) % 1 + 1) % 1
      const x = driftX * width
      const y = driftY * height
      const breathe = 0.42 + 0.58 * (0.5 + 0.5 * Math.sin(nowSec * m.twinkleSpeed + m.phase))
      const alpha = (0.14 + m.radius * 0.14) * breathe * options.activeOpacity

      if (m.halo) {
        drawRadialBloom(
          ctx,
          x,
          y,
          m.radius * 7.5,
          `rgba(247, 244, 236, ${(alpha * 0.45).toFixed(3)})`,
          `rgba(237, 230, 214, ${(alpha * 0.12).toFixed(3)})`,
        )
      }

      ctx.fillStyle = m.warmth > 0.55
        ? `rgba(237, 230, 214, ${alpha.toFixed(3)})`
        : `rgba(255, 255, 255, ${alpha.toFixed(3)})`
      ctx.beginPath()
      ctx.arc(x, y, m.radius, 0, Math.PI * 2)
      ctx.fill()
    }
    ctx.restore()
  }

  function drawSilkWave(now: number): void {
    if (burstStarted === undefined) return
    const progress = (now - burstStarted) / options.rippleDurationMs
    if (progress >= 1) {
      burstStarted = undefined
      return
    }

    // One ultra-smooth arched silk ribbon gliding across upper atmosphere above composer
    const ribbonT = smoothstep(0.0, 0.94, progress)
    const ribbonFade = smoothstep(0.0, 0.14, progress) * (1 - smoothstep(0.76, 1.0, progress))
    drawSilkRibbon(
      ctx,
      ribbonT,
      ribbonFade * 0.72,
      [-width * 0.05, height * 0.28],
      [width * 0.28, height * 0.06],
      [width * 0.72, height * 0.32],
      [width * 1.05, height * 0.14],
    )

    // Subtle soft atmospheric ring echo centered gently behind hero wordmark
    const haloFade = smoothstep(0.18, 0.42, progress) * (1 - smoothstep(0.65, 0.96, progress))
    if (haloFade > 0.01) {
      const cx = width * 0.5
      const cy = Math.max(120, height * 0.28)
      const r = 60 + smoothstep(0.18, 0.96, progress) * Math.min(width, height) * 0.16
      ctx.save()
      ctx.strokeStyle = `rgba(237, 230, 214, ${(haloFade * 0.09).toFixed(3)})`
      ctx.lineWidth = 1
      ctx.beginPath()
      ctx.arc(cx, cy, r, 0, Math.PI * 2)
      ctx.stroke()
      ctx.restore()
    }
  }

  function frame(now: number): void {
    rafId = 0
    if (!drawable()) return
    if (!moving()) {
      ctx.clearRect(0, 0, width, height)
      return
    }
    ctx.clearRect(0, 0, width, height)
    drawStardustLayer(now / 1000)
    drawSilkWave(now)
    rafId = requestAnimationFrame(frame)
  }

  function kick(): void {
    if (rafId !== 0) return
    if (!disposed && !document.hidden && visible && moving() && width > 0 && height > 0) {
      rafId = requestAnimationFrame(frame)
    }
  }

  function resize(): void {
    width = canvas.clientWidth
    height = canvas.clientHeight
    const dpr = Math.min(devicePixelRatio, 2)
    canvas.width = Math.round(width * dpr)
    canvas.height = Math.round(height * dpr)
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    kick()
  }

  function scheduleResize(): void {
    if (disposed || layoutRaf !== 0) return
    layoutRaf = requestAnimationFrame(() => {
      layoutRaf = 0
      resize()
    })
  }

  function play(): void {
    if (!moving() || !drawable()) return
    burstStarted = performance.now()
    kick()
  }

  const sizeObserver = new ResizeObserver(scheduleResize)
  const visibilityObserver = new IntersectionObserver(([entry]) => {
    visible = entry?.isIntersecting ?? false
    if (visible) kick()
  })
  const onVisibility = () => {
    if (!document.hidden) kick()
  }

  sizeObserver.observe(canvas)
  visibilityObserver.observe(canvas)
  document.addEventListener('visibilitychange', onVisibility)
  media.addEventListener('change', onVisibility)
  resize()

  const dispose = (): void => {
    disposed = true
    cancelAnimationFrame(rafId)
    cancelAnimationFrame(layoutRaf)
    rafId = layoutRaf = 0
    burstStarted = undefined
    sizeObserver.disconnect()
    visibilityObserver.disconnect()
    document.removeEventListener('visibilitychange', onVisibility)
    media.removeEventListener('change', onVisibility)
  }

  return { play, dispose }
}
