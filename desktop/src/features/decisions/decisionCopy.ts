import type { DecisionKind } from '../team/events'

export type CardKind = DecisionKind | 'permissao'

export const UNDO_WINDOW_MS = 8000
export const OTHER_OPTION = 'Outro…'
export const PANEL_TITLE = 'Decisões'
export const RESOLVED_TITLE = 'Resolvidas agora'
export const UNDO_LABEL = 'Desfazer'
export const EMPTY_TITLE = 'Nada esperando você'
export const EMPTY_TEXT = 'Os agentes seguem sozinhos. Avisamos aqui quando alguém parar.'
export const CHOOSE_HINT = 'escolha uma opção'
export const CHOOSE_LABEL = 'Escolher'
export const ANSWER_LABEL = 'Responder'
export const OTHER_PLACEHOLDER = 'Sua resposta'

export const PERMISSION_ALLOW = 'allow'
export const PERMISSION_DENY = 'deny'

export const KIND_LABELS: Record<CardKind, string> = {
  aprovacao: 'Aprovação',
  escolha: 'Escolha',
  pergunta: 'Pergunta',
  revisao: 'Revisão',
  permissao: 'Permissão',
}

export type ActionVariant = 'approve' | 'danger' | 'quiet'

export interface CardAction {
  id: string
  label: string
  variant: ActionVariant
  answer: string
  message: (agent: string, subject: string) => string
  disabled?: boolean
}

const APPROVE_ACTION: CardAction = {
  id: 'approve',
  label: 'Aprovar',
  variant: 'approve',
  answer: 'Aprovado',
  message: (agent, subject) => `${agent} pode seguir: ${subject}`,
}

const DENY_ACTION: CardAction = {
  id: 'deny',
  label: 'Negar',
  variant: 'danger',
  answer: 'Negado',
  message: (agent, subject) => `${subject} negado · ${agent} vai procurar outro caminho`,
}

const ALWAYS_ACTION: CardAction = {
  id: 'always',
  label: 'Sempre neste projeto',
  variant: 'quiet',
  answer: 'Aprovado, e vale sempre neste projeto',
  message: (_agent, subject) => `${subject} liberado sempre neste projeto`,
}

const PERMISSION_ACTIONS: readonly CardAction[] = [
  { ...APPROVE_ACTION, answer: PERMISSION_ALLOW },
  { ...DENY_ACTION, answer: PERMISSION_DENY },
]

export function actionsFor(kind: CardKind, agent: string): CardAction[] {
  switch (kind) {
    case 'aprovacao':
      return [APPROVE_ACTION, DENY_ACTION, ALWAYS_ACTION]
    case 'revisao':
      return [
        {
          id: 'approve-batch',
          label: 'Aprovar lote',
          variant: 'approve',
          answer: 'Lote aprovado',
          message: () => 'Lote aprovado',
        },
        {
          id: 'open-diff',
          label: 'Abrir diff',
          variant: 'quiet',
          answer: '',
          message: () => '',
          disabled: true,
        },
        {
          id: 'return',
          label: `Devolver ao ${agent}`,
          variant: 'quiet',
          answer: 'Devolvido para ajustes',
          message: () => `Devolvido a ${agent}`,
        },
      ]
    case 'permissao':
      return [...PERMISSION_ACTIONS]
    case 'escolha':
    case 'pergunta':
      return []
  }
}

export function optionMessage(kind: DecisionKind, agent: string, answer: string): string {
  return kind === 'escolha' ? `${agent} segue com: ${answer}` : `${agent} vai usar: ${answer}`
}
