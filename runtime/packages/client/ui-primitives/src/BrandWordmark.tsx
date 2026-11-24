import type { IconProps } from './icons/props.ts'

/** Display options for the AI Carnival brand wordmark. */
export interface BrandWordmarkProps extends IconProps {
  /** Whether to include the leading Circus Wheel emblem; defaults to true. */
  includeMark?: boolean | undefined
}

const WHEEL_ANGLES = [0, 45, 90, 135, 180, 225, 270, 315] as const

/**
 * Render the AI Carnival editorial serif wordmark paired with the Circus Ferris Wheel emblem.
 * @param props.size - height in px (default 24).
 * @param props.className - extra class for layout placement.
 * @param props.includeMark - whether to include the leading emblem mark.
 * @returns the AI Carnival wordmark svg.
 */
export function BrandWordmark({ size = 24, className, includeMark = true }: BrandWordmarkProps) {
  const width = includeMark ? 116 : 82
  return (
    <svg
      width={(size * width) / 24}
      height={size}
      className={className}
      viewBox={includeMark ? '0 0 116 24' : '32 0 82 24'}
      fill="none"
      aria-hidden="true"
    >
      {includeMark && (
        <g>
          <path
            d="M12 11.4L6.6 21.5M12 11.4L17.4 21.5M5 21.5H19"
            stroke="currentColor"
            strokeWidth="1.65"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <circle cx="12" cy="10.4" r="7.8" stroke="currentColor" strokeWidth="1.5" />
          <circle cx="12" cy="10.4" r="4.6" stroke="currentColor" strokeWidth="0.9" opacity="0.6" />
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
          <circle cx="12" cy="10.4" r="1.65" fill="currentColor" />
        </g>
      )}
      <text
        x="34"
        y="17"
        fill="currentColor"
        fontFamily="-apple-system, BlinkMacSystemFont, 'SF Pro Display', 'Helvetica Neue', sans-serif"
        fontSize="16"
        fontWeight="500"
        letterSpacing="-0.02em"
      >
        Carnival
      </text>
    </svg>
  )
}
