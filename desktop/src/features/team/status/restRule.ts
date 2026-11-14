import type { StatusRule } from './types'

export const restRule: StatusRule = {
  status: 'rest',
  applies: () => true,
}
