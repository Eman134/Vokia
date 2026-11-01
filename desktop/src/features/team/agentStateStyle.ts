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
