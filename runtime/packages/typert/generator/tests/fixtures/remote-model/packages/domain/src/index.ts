import type { TypertContext, TypertLookup } from '@carnival/dsh-typert-protocol'
import type { AgentId } from './types.ts'

/** Host-only live Agent object. */
export class Agent {
  constructor(readonly id: AgentId) {}
}

declare module '@carnival/dsh-typert-protocol' {
  interface TypertLookupMap {
    agent: TypertLookup<Agent, AgentId>
  }

  interface TypertContextMap {
    agent: TypertContext<AgentId>
  }
}

export type { AgentId } from './types.ts'
