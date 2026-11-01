import type { Agent } from '../team/events'

export type NowTab = 'todos' | Agent
export type AgentStatus = 'trabalhando' | 'precisa de você' | 'pausado' | 'livre'

export const NOW_TAB_LABEL = 'Acontecendo agora'
export const ALL_TAB_LABEL = 'Todos'
export const PAUSE_LABEL = 'Pausar'
export const RESUME_LABEL = 'Retomar'
export const OPEN_CHAT_LABEL = 'Abrir conversa'
export const PAUSED_STEP = 'pausado por você'
export const NOW_TITLE = 'Agora'
export const RECENT_TITLE = 'Últimas ações'
export const QUEUE_TITLE = 'Fila'
export const DONE_TODAY_LABEL = 'Feitas hoje'
export const YOU_LABEL = 'você'
export const NO_TASK_TEXT = 'Nenhum pedido em andamento'
export const NO_RECENT_TEXT = 'Nenhuma ação ainda'
export const MINUTE_MS = 60000

export function agentTabLabel(name: string, queued: number): string {
  return queued > 0 ? `${name} ${queued} na fila` : name
}

export function elapsedLabel(since: number, now: number): string {
  return `há ${Math.max(0, Math.floor((now - since) / MINUTE_MS))} min`
}

export function messageFieldLabel(name: string): string {
  return `Mensagem para @${name} · entra na fila dele`
}

export const BADGE_BY_STATUS: Record<AgentStatus, 'trabalhando' | 'precisa de você' | 'livre' | 'backlog'> = {
  trabalhando: 'trabalhando',
  'precisa de você': 'precisa de você',
  pausado: 'backlog',
  livre: 'livre',
}
