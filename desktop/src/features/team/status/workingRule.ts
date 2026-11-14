import type { StatusRule } from './types'

export const workingRule: StatusRule = {
  status: 'working',
  applies: (input) => input.activeAgents.length > 0,
}
