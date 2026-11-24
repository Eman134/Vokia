import type { Agent, VokiaDecision, VokiaPermission } from '../team/events'
import type { ConversationScreen, FeedEntry } from '../team/timeline'
import { GENERAL_CHANNEL_NAME, TRIAGE_CHANNEL_NAME } from './channelRouting'

export type UnreadKey = string
export type UnreadState = 'read' | 'unread' | 'decision'

const THREAD_KEY_PREFIX = 'thread:'

export function threadUnreadKey(itemId: string): UnreadKey {
  return `${THREAD_KEY_PREFIX}${itemId}`
}

// Só canais, #triagem e threads têm indicador; telas de agente e "Todos" ficam de fora.
export function unreadKeyOf(screen: ConversationScreen): UnreadKey | null {
  switch (screen.kind) {
    case 'channel':
      return screen.channel === GENERAL_CHANNEL_NAME ? GENERAL_CHANNEL_NAME : String(screen.channel)
    case 'triage':
      return TRIAGE_CHANNEL_NAME
    case 'thread':
      return threadUnreadKey(screen.itemId)
    case 'all':
    case 'agent':
    case 'copy':
      return null
  }
}

// Conta como novidade só o que o time escreve: mensagem de agente e conclusão de tarefa.
export function latestIncomingTime(entries: readonly FeedEntry[]): number | null {
  let latest: number | null = null
  for (const entry of entries) {
    const incoming = (entry.type === 'message' && entry.from !== 'user') || entry.type === 'task-result'
    if (incoming && (latest === null || entry.time > latest)) latest = entry.time
  }
  return latest
}

export interface DecisionRouting {
  threadIds: ReadonlySet<string>
  triagingIds: ReadonlySet<string>
  agentItems: Readonly<Record<Agent, string | undefined>>
}

function keysForItem(itemId: string | null | undefined, routing: DecisionRouting): UnreadKey[] {
  if (itemId == null || !routing.threadIds.has(itemId)) return [GENERAL_CHANNEL_NAME]
  const keys = [threadUnreadKey(itemId)]
  if (routing.triagingIds.has(itemId)) keys.push(TRIAGE_CHANNEL_NAME)
  return keys
}

// Decisão aponta o card; permissão só o agente, então vale o card em que ele está trabalhando. Sem thread visível, cai em #geral.
export function pendingDecisionKeys(
  decisions: readonly Pick<VokiaDecision, 'itemId'>[],
  permissions: readonly Pick<VokiaPermission, 'agent'>[],
  routing: DecisionRouting,
): Set<UnreadKey> {
  const keys = new Set<UnreadKey>()
  for (const decision of decisions) {
    for (const key of keysForItem(decision.itemId, routing)) keys.add(key)
  }
  for (const permission of permissions) {
    for (const key of keysForItem(routing.agentItems[permission.agent], routing)) keys.add(key)
  }
  return keys
}
