import type { StatusRule } from './types'

export const errorRule: StatusRule = {
  status: 'error',
  applies: (input) => input.lastIsError,
}
