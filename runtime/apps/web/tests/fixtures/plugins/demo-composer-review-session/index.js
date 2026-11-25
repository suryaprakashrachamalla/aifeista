/** Host configuration for the session-specific composer hint. */
import z from '@carnival/schemastery'

/** Copy a Session ID into `sessionId` to give its composer the review checklist. */
export const Config = z.object({
  sessionId: z.string().default('').volatile(),
})

/** Publish config to the client half without mounting a Host service. */
export function apply() {}
