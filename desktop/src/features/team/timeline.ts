import type { Agent, FeedNotice, FeedPermissionResult, VokiaError, VokiaMessage } from './events'
import type { Channel } from '../channels/channelsApi'
import { channelOf, GENERAL_CHANNEL_NAME } from '../channels/channelRouting'

export type FeedEntry = VokiaMessage | VokiaError | FeedNotice | FeedPermissionResult

export type TimelineItem =
  | { type: 'entry'; start: number; entry: FeedEntry }
  | { type: 'group'; start: number; messages: VokiaMessage[] }

// Só um fio com muitas mensagens entre os mesmos agentes recolhe; até este número ele fica aberto.
const GROUP_COLLAPSE_SIZE = 5

export function collapsesGroup(messages: readonly VokiaMessage[]): boolean {
  return messages.length > GROUP_COLLAPSE_SIZE
}

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

export type ConversationScreen =
  | { kind: 'all' }
  | { kind: 'agent'; agent: Agent }
  | { kind: 'channel'; channel: number | typeof GENERAL_CHANNEL_NAME }

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
export function screenEntries(
  feed: readonly FeedEntry[],
  screen: ConversationScreen,
  channels: readonly Channel[],
): readonly FeedEntry[] {
  switch (screen.kind) {
    case 'all':
      return feed
    case 'agent':
      return feed.filter((entry) => entryInvolves(entry, screen.agent))
    case 'channel':
      return feed.filter((entry) => entryInChannel(entry, screen.channel, channels))
  }
}

// #geral recebe o que não tem canal específico, os pedidos de permissão (E7) e os avisos e erros, que não pertencem a canal nenhum;
// canal específico recebe só as suas mensagens.
function entryInChannel(entry: FeedEntry, target: number | typeof GENERAL_CHANNEL_NAME, channels: readonly Channel[]): boolean {
  if (entry.type === 'permission-result' || entry.type === 'notice' || entry.type === 'error') {
    return target === GENERAL_CHANNEL_NAME
  }
  if (entry.type !== 'message') {
    return false
  }
  const channel = channelOf(entry, channels)
  return target === GENERAL_CHANNEL_NAME ? channel === null : channel === target
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
