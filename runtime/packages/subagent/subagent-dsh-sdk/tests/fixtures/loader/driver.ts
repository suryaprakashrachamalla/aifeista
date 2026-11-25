#!/usr/bin/env node
/** Test driver: one delegation turn through a headless Loader composition. */

import { resolveConfigPath } from '@carnival/dsh-app-boot'
import type { SessionId } from '@carnival/dsh-session'
import { runFixtureTurn } from '@carnival/dsh-loader-smoke'
import { bootProductionProfile } from '../../../../../test-support/loader-smoke/tests/fixtures/production-profile.ts'
import { brandString } from '@carnival/dsh-brand'

const configPath = process.argv[2]
if (configPath === undefined) throw new Error('sdk-subagent cwd driver requires a config path')

const ctx = await bootProductionProfile({
  binName: 'sdk-subagent-cwd-e2e',
  profile: 'headless',
  overlayPaths: [resolveConfigPath(configPath, undefined)],
})
try {
  const result = await runFixtureTurn(ctx, { task: 'delegate' })
  const parent = ctx.agents.get(brandString<SessionId>(result.sessionId))
  if (parent === undefined) throw new Error('delegating parent is unavailable')
  await ctx.subagents.waitForTreeWork(parent)
  await parent.whenIdle()
  await ctx.sessions.flush(parent.session)
} finally {
  await ctx.fiber.dispose()
}
