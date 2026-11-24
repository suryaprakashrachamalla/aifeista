import type { IconProps } from './icons/props.ts'

/** Native viewBox of {@link FISH_LOGO_PATH} (width and height in user units). */
export const FISH_LOGO_VIEWBOX = { width: 24, height: 24 }

/** Single-path Circus Ferris Wheel fallback silhouette for downstream icon consumers. */
export const FISH_LOGO_PATH =
  'M12 2.6a8.2 8.2 0 1 0 0 16.4 8.2 8.2 0 0 0 0-16.4Zm0 1.6a6.6 6.6 0 1 1 0 13.2 6.6 6.6 0 0 1 0-13.2ZM12 12.2 6.6 22h1.9l3.5-6.6L15.5 22h1.9L12 12.2Z'

const WHEEL_ANGLES = [0, 45, 90, 135, 180, 225, 270, 315] as const

/**
 * Render the AI Carnival Circus Ferris Wheel emblem.
 * @param props.size - width/height in px (default 24).
 * @param props.className - extra class for layout placement.
 * @returns the static AI Carnival Circus Wheel svg.
 */
export function FishLogo({ size = 24, className }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      className={className}
      viewBox={`0 0 ${FISH_LOGO_VIEWBOX.width} ${FISH_LOGO_VIEWBOX.height}`}
      fill="none"
      aria-hidden="true"
    >
      {/* Grounded A-frame stand */}
      <path
        d="M12 11.4L6.6 21.5M12 11.4L17.4 21.5M5 21.5H19"
        stroke="currentColor"
        strokeWidth="1.65"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      {/* Ferris wheel rims */}
      <circle cx="12" cy="10.4" r="7.8" stroke="currentColor" strokeWidth="1.5" />
      <circle cx="12" cy="10.4" r="4.6" stroke="currentColor" strokeWidth="0.9" opacity="0.6" />
      {/* Spokes & gondola cabins */}
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
      {/* Center axle hub */}
      <circle cx="12" cy="10.4" r="1.65" fill="currentColor" />
    </svg>
  )
}
