import { PAUSE_THRESHOLD } from '../usage/usagePlans'
import type { AgentProfile } from './useTeam'

export type TeamStatus = 'paused' | 'error' | 'working' | 'done' | 'rest'

export const STATUS_PRIORITY: readonly TeamStatus[] = ['paused', 'error', 'working', 'done', 'rest']
export const DONE_HOLD_MS = 15000

export interface StatusCopy {
  status: string
  detail: string
  placeholder: string
}

export const STATUS_COPY: Record<Exclude<TeamStatus, 'working'>, StatusCopy> = {
  paused: {
    status: 'Time pausado',
    detail: 'retome pela barra',
    placeholder: 'Peça algo ao projeto ou mencione @alguém',
  },
  error: { status: 'Algo quebrou', detail: 'o último pedido falhou', placeholder: 'Explique o que esperava' },
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
  working: 'stroke-ok',
  done: 'stroke-ok',
  rest: 'stroke-faint',
}

export interface TeamStatusInput {
  fiveHour: number | null
  lastIsError: boolean
  activeAgents: AgentProfile[]
  doneUntil: number
  now: number
}

export function resolveTeamStatus(input: TeamStatusInput): TeamStatus {
  const active: Record<TeamStatus, boolean> = {
    paused: input.fiveHour !== null && input.fiveHour >= PAUSE_THRESHOLD,
    error: input.lastIsError,
    working: input.activeAgents.length > 0,
    done: input.now < input.doneUntil,
    rest: true,
  }
  return STATUS_PRIORITY.find((status) => active[status]) ?? 'rest'
}

export function workingCopy(agents: readonly AgentProfile[]): StatusCopy {
  if (agents.length > 1) {
    return {
      status: `${agents.length} em paralelo`,
      detail: 'várias coisas ao mesmo tempo',
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
