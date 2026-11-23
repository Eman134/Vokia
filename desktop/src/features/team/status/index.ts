import type { AgentProfile } from '../teamApi'
import { errorSummary } from '../errorSummary'
import { doneRule } from './doneRule'
import { errorRule } from './errorRule'
import { needsYouRule } from './needsYouRule'
import { pausedRule } from './pausedRule'
import { restRule } from './restRule'
import type { StatusCopy, StatusRule, TeamStatus, TeamStatusInput } from './types'
import { workingRule } from './workingRule'

export type { StatusCopy, StatusRule, TeamStatus, TeamStatusInput } from './types'

export const STATUS_RULES: readonly StatusRule[] = [
  pausedRule,
  errorRule,
  needsYouRule,
  workingRule,
  doneRule,
  restRule,
]
export const STATUS_PRIORITY: readonly TeamStatus[] = STATUS_RULES.map((rule) => rule.status)
export const DONE_HOLD_MS = 15000

const ERROR_PLACEHOLDER = 'Explique o que esperava'

export const STATUS_COPY: Record<Exclude<TeamStatus, 'working' | 'needs-you' | 'error'>, StatusCopy> = {
  paused: {
    status: 'Time pausado',
    detail: 'retome pela barra',
    placeholder: 'Peça algo ao projeto ou mencione @alguém',
  },
  done: { status: 'Pedido concluído', detail: 'tudo certo', placeholder: 'Peça a próxima feature' },
  rest: {
    status: 'Tudo em dia',
    detail: 'nada rodando',
    placeholder: 'Peça algo ao projeto ou mencione @alguém',
  },
}

export const STATUS_RING: Record<TeamStatus, string> = {
  paused: 'stroke-danger',
  error: 'stroke-danger',
  'needs-you': 'stroke-danger',
  working: 'stroke-ok',
  done: 'stroke-ok',
  rest: 'stroke-faint',
}

export function resolveTeamStatus(input: TeamStatusInput): TeamStatus {
  return STATUS_RULES.find((rule) => rule.applies(input))?.status ?? 'rest'
}

// O erro real vai no título do status; sem mensagem o título fica vazio, sem texto fixo.
export function errorCopy(message: string): StatusCopy {
  return { status: errorSummary(message).title, detail: '', placeholder: ERROR_PLACEHOLDER }
}

// O contador de decisões já mostra quantas esperam; repetir aqui só polui a barra.
export function needsYouCopy(): StatusCopy {
  return {
    status: 'Precisa de você',
    detail: '',
    placeholder: 'Responda ou mencione @alguém',
  }
}

export function workingCopy(agents: readonly AgentProfile[]): StatusCopy {
  if (agents.length > 1) {
    return {
      status: `${agents.length} em paralelo`,
      detail: '',
      placeholder: 'Peça algo e escolha o projeto',
    }
  }
  const [agent] = agents
  return {
    status: `${agent.name} trabalhando`,
    detail: 'um agente',
    placeholder: `Mensagem para @${agent.name}`,
  }
}
