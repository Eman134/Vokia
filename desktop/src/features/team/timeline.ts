import type { Agent, FeedNotice, FeedPermissionResult, VokiaError, VokiaMessage } from './events'

export type FeedEntry = VokiaMessage | VokiaError | FeedNotice | FeedPermissionResult

export type TimelineItem =
  | { type: 'entry'; start: number; entry: FeedEntry }
  | { type: 'group'; start: number; messages: VokiaMessage[] }

export function isBetweenAgents(message: VokiaMessage): boolean {
  return message.from !== 'user' && message.to !== 'user'
}

export function groupParticipants(messages: readonly VokiaMessage[]): Agent[] {
  const participants: Agent[] = []
  for (const message of messages) {
    for (const speaker of [message.from, message.to]) {
      if (speaker !== 'user' && !participants.includes(speaker)) {
        participants.push(speaker)
      }
    }
  }
  return participants
}

export type ConversationScreen = { kind: 'all' } | { kind: 'agent'; agent: Agent }

export const ALL_SCREEN: ConversationScreen = { kind: 'all' }
export const ALL_SCREEN_LABEL = 'Todos'

export function entryInvolves(entry: FeedEntry, agent: Agent): boolean {
  switch (entry.type) {
    case 'message':
      return entry.from === agent || entry.to === agent
    case 'permission-result':
      return entry.agent === agent
    case 'notice':
    case 'error':
      return false
  }
}

// Filtra antes de agrupar: na tela de um agente, um grupo misto mostra só as mensagens dele (P4).
export function screenEntries(feed: readonly FeedEntry[], screen: ConversationScreen): readonly FeedEntry[] {
  return screen.kind === 'all' ? feed : feed.filter((entry) => entryInvolves(entry, screen.agent))
}

export function buildTimeline(feed: readonly FeedEntry[]): TimelineItem[] {
  const items: TimelineItem[] = []
  feed.forEach((entry, index) => {
    const last = items.at(-1)
    if (entry.type !== 'message' || !isBetweenAgents(entry)) {
      items.push({ type: 'entry', start: index, entry })
    } else if (last?.type === 'group') {
      last.messages.push(entry)
    } else {
      items.push({ type: 'group', start: index, messages: [entry] })
    }
  })
  return items
}
