import type UiBadge from '../../ui/UiBadge.vue'
import type { BacklogCardAction, BacklogColumn, BacklogState } from './backlogTypes'

type BadgeStatus = InstanceType<typeof UiBadge>['$props']['status']
export type ActionLabel = 'Triar' | 'Revisar'

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
export const COLUMN_CHIP: Partial<Record<BacklogColumn, { label: string; status: BadgeStatus }>> = {
  Ready: { label: 'ready', status: 'lote' },
  'In progress': { label: 'em andamento', status: 'lote' },
  'In review': { label: 'em revisão', status: 'precisa de você' },
  Done: { label: 'feito', status: 'trabalhando' },
}

export const STATE_ACTION: Partial<Record<BacklogState, ActionLabel>> = {
  vazio: 'Triar',
  proposto: 'Revisar',
}
export const ACTION_ID: Record<ActionLabel, BacklogCardAction> = {
  Triar: 'triar',
  Revisar: 'revisar',
}
export const ACTION_TITLE: Record<BacklogCardAction, string> = {
  triar: 'Mandar para a triagem da líder',
  revisar: 'Ver a proposta do plano',
  vincular: 'Vincular plano',
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
