import type { GoalState, VokiaGoalState } from './events'

const GOAL_STATE_LABEL: Record<GoalState, string> = {
  atingido: 'ATINGIDO',
  incompleto: 'INCOMPLETO',
  'sem-avanco': 'SEM AVANÇO',
  repetindo: 'REPETIÇÃO',
}

export function goalNotice(event: VokiaGoalState): string | null {
  if (event.state === null) {
    return event.reason === '' ? null : event.reason
  }
  const verdict = `Verificador: ${GOAL_STATE_LABEL[event.state]}.`
  return event.reason === '' ? verdict : `${verdict} ${event.reason}`
}
