import { planProgress, type Progress } from '../artifacts/planProgress'
import { COLUMN_CHIP } from './backlogLabels'
import type { BacklogColumn, BacklogItem, ScriptBlock } from './backlogTypes'
import type { OrchTaskStage, TaskStageState } from '../orchestration/orchestrationTypes'
import type { ParsedProposal } from './planProposal'

export type BreakdownStatus = 'proposta' | 'ready' | 'em andamento' | 'em revisão' | 'feito'

export interface BreakdownStep {
  id: string
  n: number
  title: string
  who?: string
  status: BreakdownStatus
  goal: string
  tasks: string[]
  script?: ScriptBlock
  stageId?: string
  layer?: string
  edge?: string
  flowState?: TaskStageState
}

// Camada e nome de cada etapa vêm do fluxo do time; o estado da etapa pesa mais que a coluna do card.
export interface StageFlow {
  stages: readonly OrchTaskStage[]
  layerName: (layerId: string) => string | undefined
  edgeLabel: (edgeId: number) => string | undefined
}

const FLOW_STATUS: Partial<Record<TaskStageState, BreakdownStatus>> = {
  em_andamento: 'em andamento',
  concluida: 'feito',
}

export interface PlanBreakdown {
  steps: BreakdownStep[]
  estimate: string
  progress?: Progress
}

const PLAN_PREFIX = 'plano-'
const TASK_SEPARATOR = ';'

const LINK_PREFIX_KIND: readonly [string, string][] = [
  ['plano-', 'plano'],
  ['adr-', 'ADR'],
  ['doc-', 'doc'],
  ['spec-', 'spec'],
]

export function linkKind(id: string): string {
  const lower = id.toLowerCase()
  return LINK_PREFIX_KIND.find(([prefix]) => lower.startsWith(prefix))?.[1] ?? 'link'
}

function statusFromColumn(column: BacklogColumn | undefined): BreakdownStatus {
  const label = column ? COLUMN_CHIP[column]?.label : undefined
  if (label === 'ready' || label === 'em andamento' || label === 'em revisão' || label === 'feito') {
    return label
  }
  return 'ready'
}

function flowDetails(stageId: string | undefined, column: BreakdownStatus, flow?: StageFlow): Pick<BreakdownStep, 'status' | 'layer' | 'edge' | 'flowState'> {
  const state = stageId === undefined ? undefined : flow?.stages.find((candidate) => candidate.itemId === stageId)
  if (state === undefined) return { status: column }
  const layer = state.layerId === null ? undefined : flow?.layerName(state.layerId)
  const edge = state.edgeId === null ? undefined : flow?.edgeLabel(state.edgeId)
  return {
    status: FLOW_STATUS[state.state] ?? column,
    flowState: state.state,
    ...(layer ? { layer } : {}),
    ...(edge ? { edge } : {}),
  }
}

function splitTasks(raw: string): string[] {
  return raw
    .split(TASK_SEPARATOR)
    .map((task) => task.trim())
    .filter((task) => task !== '')
}

function directStageNumber(stageId: string, itemId: string): number | undefined {
  const prefix = `${PLAN_PREFIX}${itemId}.`
  if (!stageId.startsWith(prefix)) return undefined
  const suffix = stageId.slice(prefix.length)
  if (!/^\d+$/.test(suffix)) return undefined
  return Number(suffix)
}

export function planSummaryLabel(count: number, estimate?: string): string {
  const noun = count === 1 ? 'micro-tarefa' : 'micro-tarefas'
  const base = `${count} ${noun}`
  const trimmedEstimate = estimate?.trim()
  return trimmedEstimate ? `${base} · ${trimmedEstimate}` : base
}

export function breakdownOf(
  item: BacklogItem,
  proposal: ParsedProposal | undefined,
  stages: readonly BacklogItem[],
  flow?: StageFlow,
): PlanBreakdown {
  const steps: BreakdownStep[] = []
  const matchedStageIds = new Set<string>()
  const lots = proposal?.lots ?? []

  for (const lot of lots) {
    const expectedStageId = `${PLAN_PREFIX}${item.id}.${lot.n}`
    const stage = stages.find((candidate) => candidate.id === expectedStageId)
    if (stage) matchedStageIds.add(stage.id)

    const who = (stage?.who?.trim() || lot.script?.who?.trim()) || undefined

    steps.push({
      id: `${item.id}.${lot.n}`,
      n: lot.n,
      title: stage?.title || lot.title,
      ...(who ? { who } : {}),
      ...flowDetails(stage?.id, stage ? statusFromColumn(stage.column) : 'proposta', flow),
      goal: lot.meta,
      tasks: splitTasks(lot.tasks),
      ...(lot.script ? { script: lot.script.block } : {}),
      ...(stage ? { stageId: stage.id } : {}),
    })
  }

  for (const stage of stages) {
    if (matchedStageIds.has(stage.id)) continue
    const n = directStageNumber(stage.id, item.id)
    if (n === undefined) continue
    const who = stage.who.trim() || undefined

    steps.push({
      id: `${item.id}.${n}`,
      n,
      title: stage.title,
      ...(who ? { who } : {}),
      ...flowDetails(stage.id, statusFromColumn(stage.column), flow),
      goal: stage.description ?? '',
      tasks: [],
      stageId: stage.id,
    })
  }

  return {
    steps,
    estimate: proposal?.estimate || item.estimate || '',
    ...(stages.length > 0 ? { progress: planProgress(stages) } : {}),
  }
}
