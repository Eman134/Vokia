import type { AgentState } from './events'

export const STATE_LABELS: Record<AgentState, string> = {
  thinking: 'pensando',
  waiting: 'esperando',
  idle: 'livre',
}

export const DOT_CLASSES: Record<AgentState, string> = {
  thinking: 'animate-pulse bg-ok',
  waiting: 'bg-warn',
  idle: 'bg-faint',
}

export const PAUSED_LABEL = 'pausado'
export const PAUSED_DOT_CLASS = 'bg-muted'

export function presenceOf(state: AgentState, paused: boolean): { label: string; dot: string } {
  return paused
    ? { label: PAUSED_LABEL, dot: PAUSED_DOT_CLASS }
    : { label: STATE_LABELS[state], dot: DOT_CLASSES[state] }
}
