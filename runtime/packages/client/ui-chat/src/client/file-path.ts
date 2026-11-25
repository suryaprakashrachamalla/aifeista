/** Gesture-time path resolution for Chat-owned file references. */
import type { Context } from '@carnival/cordis'
import type { SessionId } from '@carnival/dsh-session/types'
import { resolveWorkspacePath } from '@carnival/dsh-util-workspace-path'
import { sessionDirectory } from '@carnival/dsh-api-session-controller/client/directories'

/**
 * Bind path resolution to the originating Session without capturing its current directory.
 * @param ctx - Chat plugin context.
 * @param sessionId - Original Session identity.
 * @returns Resolver using the live working directory, falling back to the workspace root.
 */
export function sessionFilePath(ctx: Context, sessionId: SessionId): (path: string) => string {
  return path => resolveWorkspacePath(sessionDirectory(ctx.sessions, sessionId), path)
}
