import type { StatusRule } from './types'

export const needsYouRule: StatusRule = {
  status: 'needs-you',
  applies: (input) => input.pendingDecisions > 0,
}
