/** Windows provider registration through the Loader without an interactive desktop. */
import { Context } from '@carnival/cordis'
import Include from '@carnival/cordis-plugin-include'
import Loader from '@carnival/cordis-plugin-loader'
import { mountAgentLoopTestDependencies, mountAgentLoopTestHarness } from '@carnival/dsh-agent-loop-testkit'
import ComputerUseRegistry from '@carnival/dsh-computer-use'
import type { ToolCallId } from '@carnival/dsh-llm'
import type { SessionId } from '@carnival/dsh-session'
import SubprocessLocal from '@carnival/dsh-subprocess-local'
import { expect, it, onTestFinished } from 'vitest'
import * as WindowsProvider from '../src/index.ts'
import { brandString } from '@carnival/dsh-brand'

it.skipIf(process.platform !== 'win32')('loads the Windows provider and rejects a numeric window ID before native dispatch', async () => {
  const ctx = new Context()
  onTestFinished(() => ctx.fiber.dispose())
  await mountAgentLoopTestDependencies(ctx)
  await mountAgentLoopTestHarness(ctx)
  await ctx.plugin(ComputerUseRegistry)
  await ctx.plugin(SubprocessLocal)
  ctx.baseUrl = import.meta.url
  await ctx.plugin(Loader)
  Object.assign(ctx.loader.builtins, { include: Include, 'windows-provider': WindowsProvider })
  await ctx.loader.create({ name: 'cordis:include', config: { path: new URL('./fixtures/provider.yml', import.meta.url).href } })
  await ctx.loader.await()
  const entry = [...ctx.loader.entries()].find(entry => entry.options.id === 'windows-provider')
  if (!entry?.fiber) throw new Error('Loader did not activate the Windows provider')
  await entry.fiber.await()
  const { agent } = await ctx.agents.create({ sessionId: brandString<SessionId>('cua-windows-loader') })
  expect(ctx.tools.get('cua', agent)).toBeDefined()
  const prompt = await ctx.systemPrompt.assemble({ scope: agent })
  expect(prompt.sections.find(section => section.name === 'computer-use:dsh-cua')?.text).toContain('Windows:')
  expect(prompt.sections.find(section => section.name === 'computer-use:dsh-cua')?.text).not.toContain('useSystemInput')
  expect(prompt.sections.find(section => section.name === 'computer-use:dsh-cua')?.text).toContain('a system reminder lists any remaining keys and mouse buttons')
  const toggleMouse = prompt.sections.find(section => section.name === 'computer-use:dsh-cua')?.text
    .split('\n').find(line => line.startsWith('- toggleMouse('))
  expect(toggleMouse).toContain("toggleMouse(mouseButton: 'left' | 'right' | 'middle', direction: 'down' | 'up') -> ActionResult")
  expect(toggleMouse).toContain("Send 'up' to release a held button immediately.")
  expect(toggleMouse).toContain('press or release the specified mouse button at the current pointer position without moving it.')
  expect(toggleMouse).not.toContain('Not implemented')
  expect(JSON.stringify(ctx.tools.get('cua', agent)!.parameters)).not.toContain('useSystemInput')
  const batch = await ctx.tools.execute({ agent, signal: new AbortController().signal, calls: [{
    type: 'tool-call', id: brandString<ToolCallId>('invalid-window'), name: 'cua',
    arguments: JSON.stringify({ method: 'screenshot', params: { appHandle: 'app', windowId: 7 } }),
  }, {
    type: 'tool-call', id: brandString<ToolCallId>('unsupported-pointer'), name: 'cua',
    arguments: JSON.stringify({ method: 'drag', params: { appHandle: 'app', windowId: '7', fromX: 1, fromY: 1,
      toX: 2, toY: 2, useSystemInput: true } }),
  }] })
  expect(batch.results[1]?.result.isError).toBe(true)
  expect(JSON.stringify(batch.results[1]?.result.content)).toContain('useSystemInput')
  expect(batch.results[0]?.result.isError).toBe(true)
  expect(JSON.stringify(batch.results[0]?.result.content)).toContain('string')
  await entry.fiber.dispose()
  expect(ctx.tools.get('cua', agent)).toBeUndefined()
})
