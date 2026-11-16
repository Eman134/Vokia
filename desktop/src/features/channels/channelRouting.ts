import type { Agent, VokiaMessage } from '../team/events'
import type { Channel } from './channelsApi'

export const GENERAL_CHANNEL_NAME = 'geral'

// Mensagem com o usuário fica em #geral e na tela do agente, nunca em canal específico (P6).
export function channelOf(message: VokiaMessage, channels: readonly Channel[]): number | null {
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

// Conjunto igual, ordem ignorada (P7). #geral só nomeia o grupo que tem o roster inteiro.
export function namingChannel(
  participants: readonly Agent[],
  channels: readonly Channel[],
  roster: readonly Agent[],
): string | null {
  const match = channels.find((channel) => sameAgents(channel.members, participants))
  if (match !== undefined) {
    return match.name
  }
  return sameAgents(roster, participants) ? GENERAL_CHANNEL_NAME : null
}

function sameAgents(left: readonly Agent[], right: readonly Agent[]): boolean {
  return left.length === right.length && left.every((agent) => right.includes(agent))
}
