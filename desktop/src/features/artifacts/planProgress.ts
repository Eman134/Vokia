import { COLUMN_CHIP } from '../backlog/backlogLabels'
import { DEFAULT_COLUMN, READY_COLUMN, type BacklogColumn, type BacklogItem } from '../backlog/backlogTypes'
import { planArtifactId } from './artifactApi'

type StageChip = NonNullable<(typeof COLUMN_CHIP)[BacklogColumn]>

const PERCENT_SCALE = 100
const DONE_COLUMN: BacklogColumn = 'Done'
const PENDING_CHIP: StageChip = { label: 'pendente', status: 'backlog' }
const UNSTARTED_COLUMNS: readonly BacklogColumn[] = [DEFAULT_COLUMN, READY_COLUMN]
const STAGE_NUMBER = /^\d+$/

export interface Progress {
  done: number
  total: number
  percent: number
}

export function progressOf(done: number, total: number): Progress | undefined {
  if (total === 0) return undefined
  return { done, total, percent: Math.round((done / total) * PERCENT_SCALE) }
}

export function stagesOf(items: readonly BacklogItem[], planId: string): BacklogItem[] {
  return items.filter((item) => item.parent === planId)
}

export function isStageDone(stage: BacklogItem): boolean {
  return stage.column === DONE_COLUMN
}

export function stageChip(stage: BacklogItem): StageChip {
  return (stage.column && COLUMN_CHIP[stage.column]) || PENDING_CHIP
}

export function isStageStarted(stage: BacklogItem): boolean {
  return !UNSTARTED_COLUMNS.includes(stage.column ?? DEFAULT_COLUMN)
}

function sameTitle(a: string, b: string): boolean {
  return a.trim().toLowerCase() === b.trim().toLowerCase()
}

// Mesma regra de assign_child_ids no backend: primeiro o card de mesmo título, depois o da mesma posição.
// Depois de uma remoção, os ids dos cards não andam; só um card ainda não iniciado é herdado pela posição.
export function matchStages(itemId: string, stages: readonly BacklogItem[], titles: readonly string[]): (BacklogItem | undefined)[] {
  const prefix = `${planArtifactId(itemId)}.`
  const owned = stages.filter((stage) => stage.id.startsWith(prefix) && STAGE_NUMBER.test(stage.id.slice(prefix.length)))
  const taken = new Set<string>()
  const matched = titles.map((title) => {
    const card = owned.find((stage) => !taken.has(stage.id) && sameTitle(stage.title, title))
    if (card) taken.add(card.id)
    return card
  })
  const removed = titles.length < owned.length
  return matched.map((card, index) => {
    if (card) return card
    const byPosition = owned.find((stage) => stage.id === `${prefix}${index + 1}` && !taken.has(stage.id))
    if (!byPosition || (removed && isStageStarted(byPosition))) return undefined
    taken.add(byPosition.id)
    return byPosition
  })
}

export function planProgress(stages: readonly BacklogItem[]): Progress | undefined {
  return progressOf(stages.filter(isStageDone).length, stages.length)
}
