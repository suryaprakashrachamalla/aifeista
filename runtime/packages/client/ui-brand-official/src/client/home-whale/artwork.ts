/** Fixed artwork parameters for the AI Carnival 60fps magical star-swirl & Circus Wheel background. */

export interface CarnivalWheelArtwork {
  /** Maximum spectacle fill opacity before per-layer modulation. */
  activeOpacity: number
  /** Base phrase size in CSS pixels. */
  fontSize: number
  /** AI Carnival & harness code/brand tokens rendered inside the swirling matrix. */
  phrases: readonly string[]
  /** Target animation redraw rate. */
  frameRate: number
  /** Total choreographed 4-act spectacle duration in milliseconds. */
  rippleDurationMs: number
}

export type WhaleArtwork = CarnivalWheelArtwork

export const CARNIVAL_WHEEL_ARTWORK: CarnivalWheelArtwork = {
  activeOpacity: 0.72,
  fontSize: 9,
  phrases: [
    'Carnival',
  ],
  frameRate: 60,
  rippleDurationMs: 6400,
}

export const WHALE_ARTWORK: WhaleArtwork = CARNIVAL_WHEEL_ARTWORK
