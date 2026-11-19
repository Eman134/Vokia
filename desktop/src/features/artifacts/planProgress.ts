import { COLUMN_CHIP } from '../backlog/backlogLabels'
import type { BacklogColumn, BacklogItem } from '../backlog/backlogTypes'

type StageChip = NonNullable<(typeof COLUMN_CHIP)[BacklogColumn]>

const PERCENT_SCALE = 100
const DONE_COLUMN: BacklogColumn = 'Done'
const PENDING_CHIP: StageChip = { label: 'pendente', status: 'backlog' }

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

export function planProgress(stages: readonly BacklogItem[]): Progress | undefined {
  return progressOf(stages.filter(isStageDone).length, stages.length)
}
