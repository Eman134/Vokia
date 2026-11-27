import type { BacklogItem, Sprint } from './backlogTypes'

export const SPRINT_ALL_FILTER = '__all__'
export const SPRINT_NONE_FILTER = '__none__'

export type TimeFilter =
  | 'todos'
  | 'hoje'
  | 'ultimos-7d'
  | 'ultimos-30d'
  | 'janela-sprint'
  | 'sem-atividade-7d'

export type InteractionFilter =
  | 'todas'
  | 'perguntas-abertas'
  | 'alta-5+'
  | 'com-interacoes'
  | 'sem-interacoes'

export const DEFAULT_TIME_FILTER: TimeFilter = 'todos'
export const DEFAULT_INTERACTION_FILTER: InteractionFilter = 'todas'
export const HIGH_INTERACTION_THRESHOLD = 5

const DAY_MS = 24 * 60 * 60 * 1000
const ACTIVITY_DATE_PATTERN = /^(\d{2})\/(\d{2})\/(\d{4}),\s*(\d{2}):(\d{2})(?::(\d{2}))?$/

export const TIME_FILTER_OPTIONS: readonly { value: TimeFilter; label: string }[] = [
  { value: 'todos', label: 'Tempo: todos' },
  { value: 'hoje', label: 'Hoje' },
  { value: 'ultimos-7d', label: 'Últimos 7 dias' },
  { value: 'ultimos-30d', label: 'Últimos 30 dias' },
  { value: 'janela-sprint', label: 'Janela da sprint ativa' },
  { value: 'sem-atividade-7d', label: 'Sem atividade há 7+ dias' },
]

export const INTERACTION_FILTER_OPTIONS: readonly { value: InteractionFilter; label: string }[] = [
  { value: 'todas', label: 'Interações: todas' },
  { value: 'perguntas-abertas', label: 'Com perguntas abertas' },
  { value: 'alta-5+', label: 'Alta interação (5+)' },
  { value: 'com-interacoes', label: 'Com interações' },
  { value: 'sem-interacoes', label: 'Sem interações' },
]

export function parseActivityTimestamp(at: string): number | null {
  const match = ACTIVITY_DATE_PATTERN.exec(at.trim())
  if (!match) return null
  const [, day, month, year, hour, minute, second = '0'] = match
  const parsed = new Date(
    Number(year),
    Number(month) - 1,
    Number(day),
    Number(hour),
    Number(minute),
    Number(second),
  ).getTime()
  return Number.isNaN(parsed) ? null : parsed
}

export function effectiveLastActivityAt(item: BacklogItem): number | null {
  let latest: number | null = typeof item.lastActivityAt === 'number' ? item.lastActivityAt : null
  for (const entry of item.activity ?? []) {
    const parsed = parseActivityTimestamp(entry.at)
    if (parsed !== null && (latest === null || parsed > latest)) {
      latest = parsed
    }
  }
  return latest
}

export function openQuestionsCount(item: BacklogItem): number {
  return (item.questions ?? []).filter((question) => !question.selected?.trim()).length
}

export function matchesSprintFilter(item: BacklogItem, filter: string): boolean {
  if (filter === SPRINT_ALL_FILTER) return true
  if (filter === SPRINT_NONE_FILTER) return !item.sprintId
  return item.sprintId === filter
}

export function matchesTimeFilter(
  item: BacklogItem,
  filter: TimeFilter,
  activeSprint: Sprint | null,
  now: number = Date.now(),
): boolean {
  if (filter === 'todos') return true
  const last = effectiveLastActivityAt(item)
  if (filter === 'hoje') {
    if (last === null) return false
    const startOfToday = new Date(now)
    startOfToday.setHours(0, 0, 0, 0)
    return last >= startOfToday.getTime()
  }
  if (filter === 'ultimos-7d') {
    return last !== null && last >= now - 7 * DAY_MS
  }
  if (filter === 'ultimos-30d') {
    return last !== null && last >= now - 30 * DAY_MS
  }
  if (filter === 'janela-sprint') {
    if (!activeSprint) return false
    if (activeSprint.startsAt == null && activeSprint.endsAt == null) {
      return item.sprintId === activeSprint.id
    }
    if (last === null) return false
    const start = activeSprint.startsAt ?? Number.NEGATIVE_INFINITY
    const end = activeSprint.endsAt ?? Number.POSITIVE_INFINITY
    return last >= start && last <= end
  }
  // sem-atividade-7d
  return last === null || last < now - 7 * DAY_MS
}

export function matchesInteractionFilter(item: BacklogItem, filter: InteractionFilter): boolean {
  if (filter === 'todas') return true
  const interactions = item.interactionCount ?? 0
  const openQuestions = openQuestionsCount(item)
  if (filter === 'perguntas-abertas') return openQuestions > 0
  if (filter === 'alta-5+') return interactions >= HIGH_INTERACTION_THRESHOLD
  if (filter === 'com-interacoes') return interactions > 0 || openQuestions > 0
  return interactions === 0 && openQuestions === 0
}
