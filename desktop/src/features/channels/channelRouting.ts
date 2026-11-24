import type { Agent, VokiaMessage } from '../team/events'
import type { Channel } from './channelsApi'

export const GENERAL_CHANNEL_NAME = 'geral'
export const TRIAGE_CHANNEL_NAME = 'triagem'

// Mensagem com o usuário fica em #geral e na tela do agente, nunca em canal específico (P6).
export function channelOf(message: VokiaMessage, channels: readonly Channel[]): number | null {
  if (message.channelId !== undefined) {
    return channels.some((channel) => channel.id === message.channelId) ? message.channelId : null
  }
  if (message.from === 'user' || message.to === 'user') {
    return null
  }
  const participants: readonly Agent[] = [message.from, message.to]
  let best: Channel | null = null
  for (const channel of channels) {
    if (!participants.every((agent) => channel.members.includes(agent))) {
      continue
    }
    if (best === null || channel.members.length < best.members.length || (channel.members.length === best.members.length && channel.id < best.id)) {
      best = channel
    }
  }
  return best?.id ?? null
}
