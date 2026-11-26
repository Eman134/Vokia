import type { Agent, AgentState } from './events'
import { formatCostUsd, formatThinkingDuration } from './messageUsageFormat'
import type { FeedEntry } from './timeline'

export interface ItemUsage {
  costUsd: number
  workMs: number
  turns: number
}

export interface ActiveThinkingInput {
  itemId: string
  projectId: number | null | undefined
  states: Readonly<Partial<Record<Agent, AgentState>>>
  stateSince: Readonly<Partial<Record<Agent, number>>>
  stateItem: Readonly<Partial<Record<Agent, string | undefined>>>
  lastProject: Readonly<Partial<Record<Agent, number | null>>>
  now?: number
}

export const EMPTY_ITEM_USAGE: ItemUsage = { costUsd: 0, workMs: 0, turns: 0 }

function finitePositive(value: number | undefined): number {
  return value !== undefined && Number.isFinite(value) && value > 0 ? value : 0
}

function matchesItem(entryItemId: string | undefined, itemId: string): boolean {
  return entryItemId === itemId || (entryItemId?.startsWith(`${itemId}.`) ?? false)
}

// Soma custo e tempo de trabalho de todas as mensagens do item (e suas etapas delegadas), de qualquer agente; só turnos com uso ou duração contam.
export function itemUsageIn(feed: readonly FeedEntry[], itemId: string): ItemUsage {
  const total = { costUsd: 0, workMs: 0, turns: 0 }
  for (const entry of feed) {
    if (entry.type !== 'message' || !matchesItem(entry.itemId, itemId)) continue
    const cost = finitePositive(entry.usage?.costUsd)
    const thinkingMs = finitePositive(entry.activity?.thinkingMs)
    const toolsMs = (entry.activity?.tools ?? []).reduce((sum, tool) => sum + finitePositive(tool.durationMs), 0)
    const work = Math.max(finitePositive(entry.usage?.durationMs), thinkingMs + toolsMs)
    if (cost === 0 && work === 0) continue
    total.costUsd += cost
    total.workMs += work
    total.turns += 1
  }
  return total
}

// Soma o tempo pensando ao vivo de agentes e réplicas atualmente em 'thinking' no card e projeto ativos.
export function activeThinkingMsForItem(input: ActiveThinkingInput): number {
  const { itemId, projectId, states, stateSince, stateItem, lastProject, now = Date.now() } = input
  if (projectId === null || projectId === undefined) return 0
  let total = 0
  for (const [agent, state] of Object.entries(states)) {
    if (state !== 'thinking') continue
    if (stateItem[agent] !== itemId) continue
    if (lastProject[agent] !== projectId) continue
    const since = stateSince[agent]
    if (since === undefined || !Number.isFinite(since)) continue
    total += Math.max(0, now - since)
  }
  return total
}

const USAGE_SEPARATOR = ' · '

// Texto curto para o card ("$0.75 · 1m 5s"); exibe se houver turno medido ou tempo ativo, sem gerar '1s' falso com workMs === 0.
export function formatItemUsage(usage: ItemUsage): string | null {
  if (usage.turns === 0 && usage.workMs <= 0) return null
  const duration = usage.workMs > 0 ? formatThinkingDuration(usage.workMs) : null
  const parts = [formatCostUsd(usage.costUsd), duration].filter((part): part is string => part !== null)
  return parts.length > 0 ? parts.join(USAGE_SEPARATOR) : null
}
