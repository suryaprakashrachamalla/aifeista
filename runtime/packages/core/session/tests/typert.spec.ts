import { describe, expect, it } from 'vitest'
import { Context } from '@carnival/cordis'
import SessionStore, { type SessionId } from '@carnival/dsh-session'
import TypertRegistry from '@carnival/dsh-typert-registry'
import { brandString } from '@carnival/dsh-brand'

describe('Session Typert provider', () => {
  it('contributes live Session lookup in either service load order', async () => {
    const ctx = new Context()
    const sessionFiber = ctx.plugin(SessionStore)
    await sessionFiber
    await ctx.plugin(TypertRegistry)
    const session = ctx.sessions.create(brandString<SessionId>('remote-session'))

    const lookup = ctx.typert.lookups.get('session')
    expect(lookup).toMatchObject({
      parameter: 'session',
      wire: 'sessionId',
      resource: 'session',
      hostTypeSymbol: '@carnival/dsh-session#Session',
      wireTypeSymbol: '@carnival/dsh-session/types#SessionId',
    })
    expect(lookup?.resolve(session.id)).toBe(session)

    await sessionFiber.dispose()
    expect(ctx.typert.lookups.get('session')).toBeUndefined()
  })
})
