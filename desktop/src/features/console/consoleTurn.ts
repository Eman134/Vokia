import type { ConsoleEntry } from '../team/events'

export function currentTurn(entries: readonly ConsoleEntry[]): ConsoleEntry[] {
  const start = entries.map((entry) => entry.kind).lastIndexOf('start')
  return start === -1 ? [...entries] : entries.slice(start)
}
