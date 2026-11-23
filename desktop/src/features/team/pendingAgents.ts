import type { Channel } from '../channels/channelsApi'
import { GENERAL_CHANNEL_NAME } from '../channels/channelRouting'
import { STATE_LABELS } from './agentStateStyle'
import type { Agent, AgentState } from './events'
import type { AgentProfile } from './teamApi'
import type { ConversationScreen } from './timeline'

export type PendingState = 'thinking' | 'sending'

export interface PendingAgentEntry {
  agent: AgentProfile
  state: PendingState
  since?: number
}

export interface PendingAgentGroup {
  state: PendingState
  entries: PendingAgentEntry[]
}

export interface PendingScreenContext {
  channels: readonly Channel[]
  thread?: {
    assignee: Agent | null
    participants: readonly Agent[]
  }
}

const PENDING_STATE_ORDER: readonly PendingState[] = ['thinking', 'sending']

export function isPendingState(state: AgentState): state is PendingState {
  return state === 'thinking' || state === 'sending'
}

export function selectPendingAgents(
  agents: readonly AgentProfile[],
  states: Record<Agent, AgentState>,
): AgentProfile[] {
  return agents.filter((agent) => isPendingState(states[agent.id] ?? 'idle'))
}

export function isAgentInScreen(
  agentId: Agent,
  screen: ConversationScreen,
  context: PendingScreenContext,
): boolean {
  switch (screen.kind) {
    case 'all':
      return true
    case 'agent':
      return screen.agent === agentId
    case 'copy':
      return screen.copyId === agentId
    case 'channel': {
      if (screen.channel === GENERAL_CHANNEL_NAME) {
        return true
      }
      const channel = context.channels.find((item) => item.id === screen.channel)
      return channel?.members.includes(agentId) ?? false
    }
    case 'thread': {
      if (!context.thread) {
        return false
      }
      return context.thread.assignee === agentId || context.thread.participants.includes(agentId)
    }
  }
}

export function filterPendingAgentsForScreen(
  agents: readonly AgentProfile[],
  screen: ConversationScreen,
  context: PendingScreenContext,
): AgentProfile[] {
  return agents.filter((agent) => isAgentInScreen(agent.id, screen, context))
}

export function groupPendingAgentsByState(
  agents: readonly AgentProfile[],
  states: Record<Agent, AgentState>,
  stateSince: Partial<Record<Agent, number>> = {},
): PendingAgentGroup[] {
  const buckets: Record<PendingState, PendingAgentEntry[]> = {
    thinking: [],
    sending: [],
  }

  for (const agent of agents) {
    const state = states[agent.id] ?? 'idle'
    if (!isPendingState(state)) {
      continue
    }
    buckets[state].push({
      agent,
      state,
      since: stateSince[agent.id],
    })
  }

  return PENDING_STATE_ORDER.filter((state) => buckets[state].length > 0).map((state) => ({
    state,
    entries: buckets[state],
  }))
}

export function formatPendingStatusLabel(groups: readonly PendingAgentGroup[]): string | undefined {
  if (groups.length === 0) {
    return undefined
  }
  return groups
    .map((group) => {
      const names = group.entries.map((entry) => entry.agent.name).join(', ')
      const verb = group.entries.length === 1 ? 'está' : 'estão'
      return `${names} ${verb} ${STATE_LABELS[group.state]}`
    })
    .join('; ')
}
