import type { AgentProfile } from '../teamApi'

export type TeamStatus = 'paused' | 'error' | 'needs-you' | 'working' | 'done' | 'rest'

export interface StatusCopy {
  status: string
  detail: string
  placeholder: string
}

export interface TeamStatusInput {
  fiveHour: number | null
  lastIsError: boolean
  pendingDecisions: number
  activeAgents: AgentProfile[]
  doneUntil: number
  now: number
}

export interface StatusRule {
  status: TeamStatus
  applies: (input: TeamStatusInput) => boolean
}
