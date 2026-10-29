import type UiBadge from '../../ui/UiBadge.vue'
import type { BacklogCardAction, BacklogState, BacklogTag, KanbanStage } from './backlogTypes'

type BadgeStatus = InstanceType<typeof UiBadge>['$props']['status']
export type ActionLabel = 'Triar' | 'Revisar' | 'Ver diff'

export const STATE_CHIP: Record<BacklogState, string> = {
  vazio: 'não triado',
  triagem: 'triagem',
  gerando: 'escrevendo',
  proposto: 'proposta',
  vinculado: 'pronto',
}
export const STAGE_CHIP: Record<KanbanStage, string> = {
  ready: 'ready',
  progress: 'em andamento',
  review: 'em revisão',
  done: 'feito',
}
export const STATE_BADGE: Record<BacklogState, BadgeStatus> = {
  vazio: 'backlog',
  triagem: 'lote',
  gerando: 'lote',
  proposto: 'precisa de você',
  vinculado: 'trabalhando',
}
export const STAGE_BADGE: Record<KanbanStage, BadgeStatus> = {
  ready: 'lote',
  progress: 'lote',
  review: 'precisa de você',
  done: 'trabalhando',
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

export const STATE_ACTION: Partial<Record<BacklogState, Exclude<ActionLabel, 'Ver diff'>>> = {
  vazio: 'Triar',
  proposto: 'Revisar',
}
export const ACTION_ID: Record<ActionLabel, BacklogCardAction> = {
  Triar: 'triar',
  Revisar: 'revisar',
  'Ver diff': 'ver-diff',
}

export const FEATURE_COLOR: Record<string, string> = {
  'Pagamento via Pix': 'feature-blue',
  'Painel administrativo': 'feature-green',
  Checkout: 'feature-orange',
  'Cartão parcelado': 'feature-purple',
}
export const DEFAULT_FEATURE_COLOR = 'accent'

export const TRIAGE_STEP_COUNT = 3
export const YOU_INITIAL = 'K'

export function colorVar(token: string): string {
  return `var(--color-${token})`
}
