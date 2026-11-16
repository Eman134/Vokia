import type { AppPage } from '../../shell/railNav'
import { GENERAL_CHANNEL_NAME } from '../channels/channelRouting'
import type { Agent } from './events'
import type { ConversationScreen } from './timeline'

export const CONVERSATIONS_PAGE: AppPage = 'conversas'
export const GENERAL_SCREEN_PLACEHOLDER = 'Mensagem para o time · #geral'

// A tela aberta fica velha fora de Conversas; por isso o destino e o texto só valem nela.
export function screenAgent(page: AppPage | null, screen: ConversationScreen): Agent | null {
  return page === CONVERSATIONS_PAGE && screen.kind === 'agent' ? screen.agent : null
}

export function screenPlaceholder(
  page: AppPage | null,
  screen: ConversationScreen,
  nameOf: (agent: Agent) => string,
): string | undefined {
  if (page !== CONVERSATIONS_PAGE) return undefined
  if (screen.kind === 'agent') return `Mensagem para ${nameOf(screen.agent)}`
  return screen.kind === 'channel' && screen.channel === GENERAL_CHANNEL_NAME ? GENERAL_SCREEN_PLACEHOLDER : undefined
}
