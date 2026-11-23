import type { AppPage } from '../../shell/railNav'
import { GENERAL_CHANNEL_NAME } from '../channels/channelRouting'
import type { Channel } from '../channels/channelsApi'
import { DEFAULT_RECIPIENT } from './agents'
import type { Agent } from './events'
import type { ConversationScreen } from './timeline'

export const CONVERSATIONS_PAGE: AppPage = 'conversas'
export const GENERAL_SCREEN_PLACEHOLDER = 'Mensagem para o time · #geral'

// A tela aberta fica velha fora de Conversas; por isso o destino e o texto só valem nela.
export function screenAgent(page: AppPage | null, screen: ConversationScreen): Agent | null {
  return page === CONVERSATIONS_PAGE && screen.kind === 'agent' ? screen.agent : null
}

export interface ScreenDestination {
  to: Agent | null
  channelId: number | null
  itemId?: string | null
}

export function channelRecipient(channel: Channel, rosterOrder: readonly Agent[]): Agent | null {
  if (channel.members.includes(DEFAULT_RECIPIENT)) return DEFAULT_RECIPIENT
  return rosterOrder.find((agent) => channel.members.includes(agent)) ?? null
}

export function screenChannel(
  page: AppPage | null,
  screen: ConversationScreen,
  channels: readonly Channel[],
): Channel | null {
  if (page !== CONVERSATIONS_PAGE || screen.kind !== 'channel' || screen.channel === GENERAL_CHANNEL_NAME) return null
  return channels.find((channel) => channel.id === screen.channel) ?? null
}

export function screenDestination(
  page: AppPage | null,
  screen: ConversationScreen,
  channels: readonly Channel[],
  rosterOrder: readonly Agent[],
  mention: Agent | null,
  threadAssignee: Agent | null = null,
): ScreenDestination {
  if (page === CONVERSATIONS_PAGE && screen.kind === 'thread') {
    return {
      to: mention ?? threadAssignee ?? DEFAULT_RECIPIENT,
      channelId: null,
      itemId: screen.itemId,
    }
  }
  const channel = screenChannel(page, screen, channels)
  if (channel !== null) {
    if (mention !== null) {
      return { to: mention, channelId: channel.members.includes(mention) ? channel.id : null }
    }
    const to = channelRecipient(channel, rosterOrder)
    return to === null ? { to: null, channelId: null } : { to, channelId: channel.id }
  }
  return { to: mention ?? screenAgent(page, screen), channelId: null }
}

export function screenPlaceholder(
  page: AppPage | null,
  screen: ConversationScreen,
  nameOf: (agent: Agent) => string,
  channels: readonly Channel[] = [],
  rosterOrder: readonly Agent[] = [],
  threadTitle?: string,
): string | undefined {
  if (page !== CONVERSATIONS_PAGE) return undefined
  if (screen.kind === 'thread') {
    return threadTitle ? `Mensagem na thread de ${threadTitle}` : 'Mensagem na thread'
  }
  if (screen.kind === 'agent') return `Mensagem para ${nameOf(screen.agent)}`
  const channel = screenChannel(page, screen, channels)
  if (channel !== null) {
    const to = channelRecipient(channel, rosterOrder)
    return to === null ? `Mensagem para #${channel.name}` : `Mensagem para #${channel.name} · vai para ${nameOf(to)}`
  }
  return screen.kind === 'channel' && screen.channel === GENERAL_CHANNEL_NAME ? GENERAL_SCREEN_PLACEHOLDER : undefined
}
