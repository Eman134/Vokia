import type { StatusRule } from './types'

export const doneRule: StatusRule = {
  status: 'done',
  applies: (input) => input.now < input.doneUntil,
}
