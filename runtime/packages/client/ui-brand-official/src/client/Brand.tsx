import { BrandWordmark, FishLogo } from '@deepseek-ai/dsh-client-ui-primitives'
import type { SidebarBrandMarkOwnerProps } from '@deepseek-ai/dsh-client-ui-sidebar/client'

/**
 * Render the AI Carnival Circus Ferris Wheel mark with the size requested by its host surface.
 * @param props - Host-supplied mark presentation.
 * @returns the AI Carnival Circus Wheel mark.
 */
export function OfficialBrandMark({ size }: SidebarBrandMarkOwnerProps) {
  return <FishLogo size={size} />
}

/**
 * Render the AI Carnival name wordmark without its independently slotted mark.
 * @returns the AI Carnival name wordmark.
 */
export function OfficialBrandName() {
  return <BrandWordmark includeMark={false} />
}
