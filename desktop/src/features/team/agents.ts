import type { Agent, AgentState } from './events'
import type { AgentProfile } from './teamApi'
import { normalizeName, useRoster } from './useRoster'

export const DEFAULT_RECIPIENT: Agent = 'ember'

export function agentName(id: Agent): string {
  return useRoster().nameOf(id)
}

export function findNamedAgent(text: string): Agent | null {
  let found: { agent: Agent; index: number } | null = null
  const normalizedText = normalizeName(text)
  for (const profile of useRoster().agents) {
    const index = normalizedText.search(new RegExp(`(?<!\\p{L})${normalizeName(profile.name)}(?!\\p{L})`, 'u'))
    if (index !== -1 && (found === null || index < found.index)) {
      found = { agent: profile.id, index }
    }
  }
  return found?.agent ?? null
}

// O orquestrador não ocupa time nem camada: fica fora de qualquer lista de posicionamento ou de líder.
export function placeableAgents(agents: readonly AgentProfile[]): AgentProfile[] {
  return agents.filter((agent) => agent.id !== DEFAULT_RECIPIENT)
}

export function selectActiveAgents(agents: readonly AgentProfile[], states: Record<Agent, AgentState>): AgentProfile[] {
  return agents.filter((agent) => (states[agent.id] ?? 'idle') !== 'idle')
}
