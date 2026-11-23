import { PAUSE_THRESHOLD } from '../../usage/usagePlans'
import type { StatusRule } from './types'

export const pausedRule: StatusRule = {
  status: 'paused',
  applies: (input) =>
    Boolean(input.allPaused) || (input.fiveHour !== null && input.fiveHour >= PAUSE_THRESHOLD),
}
