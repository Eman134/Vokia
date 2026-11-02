import type UiBadge from '../../ui/UiBadge.vue'
import type { BacklogCardAction, BacklogColumn, BacklogState, BacklogTag } from './backlogTypes'

type BadgeStatus = InstanceType<typeof UiBadge>['$props']['status']
export type ActionLabel = 'Triar' | 'Revisar'

export const STATE_CHIP: Record<BacklogState, string> = {
  vazio: 'não triado',
  triagem: 'triagem',
  gerando: 'escrevendo',
  proposto: 'proposta',
  vinculado: 'pronto',
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

export const TAG_LABEL: Record<BacklogTag, string> = {
  bug: 'bug',
  urgente: 'urgente',
  ui: 'interface',
  backend: 'backend',
  ideia: 'ideia',
  divida: 'dívida técnica',
  cliente: 'pedido de cliente',
}
export const TAG_COLOR: Record<BacklogTag, string> = {
  bug: 'danger',
  urgente: 'warn',
  ui: 'feature-blue',
  backend: 'feature-green',
  ideia: 'feature-purple',
  divida: 'muted',
  cliente: 'client',
}
// O texto do pill usa o token de texto; o fundo continua com o token da cor.
export const TAG_TEXT_COLOR: Partial<Record<BacklogTag, string>> = {
  urgente: 'warn-text',
}

export const STATE_ACTION: Partial<Record<BacklogState, ActionLabel>> = {
  vazio: 'Triar',
  proposto: 'Revisar',
}
export const ACTION_ID: Record<ActionLabel, BacklogCardAction> = {
  Triar: 'triar',
  Revisar: 'revisar',
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
