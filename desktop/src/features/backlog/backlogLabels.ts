import type UiBadge from '../../ui/UiBadge.vue'
import type { TaskCardAction, BacklogColumn, BacklogOrigin, BacklogState } from './backlogTypes'
import type { BreakdownStatus } from './planBreakdown'

type BadgeStatus = InstanceType<typeof UiBadge>['$props']['status']
export type ActionLabel = 'Triar' | 'Revisar' | 'Iniciar'

export const STATE_CHIP: Record<BacklogState, string> = {
  vazio: 'não triado',
  triagem: 'triagem',
  gerando: 'escrevendo',
  proposto: 'proposta',
  vinculado: 'pronto',
}
export const PLAN_DIALOG_CHIP: Record<BacklogState, string> = {
  vazio: 'sem plano',
  triagem: 'triagem',
  gerando: 'escrevendo…',
  proposto: 'esperando você',
  vinculado: 'vinculado',
}
export const STATE_BADGE: Record<BacklogState, BadgeStatus> = {
  vazio: 'backlog',
  triagem: 'lote',
  gerando: 'lote',
  proposto: 'precisa de você',
  vinculado: 'trabalhando',
}
export const MICRO_STATUS_BADGE: Record<BreakdownStatus, BadgeStatus> = {
  proposta: 'precisa de você',
  ready: 'lote',
  'em andamento': 'lote',
  'em revisão': 'precisa de você',
  feito: 'trabalhando',
}
export const BADGE_DOT: Record<BadgeStatus, string> = {
  trabalhando: 'bg-ok',
  'precisa de você': 'bg-warn',
  livre: 'bg-faint',
  erro: 'bg-danger',
  backlog: 'bg-muted',
  lote: 'bg-accent',
}
export const AUTO_CLOSED_LABEL = 'concluída automaticamente'
export const REOPEN_LABEL = 'Reabrir'
export const REOPEN_MOTIVE_PLACEHOLDER = 'Por que reabrir?'
export const DONE_COLUMN_NAME = 'Done'
export const CLOSED_BY_AUTOMATIC = 'automatico'

export const COLUMN_CHIP: Partial<Record<BacklogColumn, { label: string; status: BadgeStatus }>> = {
  Ready: { label: 'ready', status: 'lote' },
  'In progress': { label: 'em andamento', status: 'lote' },
  'In review': { label: 'em revisão', status: 'precisa de você' },
  Done: { label: 'feito', status: 'trabalhando' },
}

export const ORIGIN_LABEL: Record<BacklogOrigin, string> = {
  usuario: 'Você',
  conversa: 'Conversa',
  agente: 'Agente',
}

export const STATE_ACTION: Partial<Record<BacklogState, ActionLabel>> = {
  vazio: 'Triar',
  proposto: 'Revisar',
}
export const ACTION_ID: Record<ActionLabel, TaskCardAction> = {
  Triar: 'triar',
  Revisar: 'revisar',
  Iniciar: 'iniciar',
}
export const ACTION_TITLE: Record<TaskCardAction, string> = {
  triar: 'Mandar para a triagem da líder',
  revisar: 'Ver a proposta do plano',
  vincular: 'Vincular plano',
  iniciar: 'Iniciar com o agente do plano',
}

export const FEATURE_COLOR: Record<string, string> = {
  'Pagamento via Pix': 'feature-blue',
  'Painel administrativo': 'feature-green',
  Checkout: 'feature-orange',
  'Cartão parcelado': 'feature-purple',
}
export const DEFAULT_FEATURE_COLOR = 'accent'

export const TRIAGE_STEP_COUNT = 3

export function colorVar(token: string): string {
  return `var(--color-${token})`
}
