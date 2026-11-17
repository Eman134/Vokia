import type { ConsoleEntry } from '../team/events'

export const CONSOLE_PREVIEW_LINES = 6
const MS_PER_SECOND = 1000
const SECONDS_PER_MINUTE = 60

export function currentTurn(entries: readonly ConsoleEntry[]): ConsoleEntry[] {
  const start = entries.map((entry) => entry.kind).lastIndexOf('start')
  return start === -1 ? [...entries] : entries.slice(start)
}

export function elapsedLabel(fromMs: number, toMs: number): string {
  const seconds = Math.max(0, Math.floor((toMs - fromMs) / MS_PER_SECOND))
  return `${Math.floor(seconds / SECONDS_PER_MINUTE)}:${String(seconds % SECONDS_PER_MINUTE).padStart(2, '0')}`
}
