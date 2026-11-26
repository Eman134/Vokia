import { formatCostUsd, formatThinkingDuration } from './messageUsageFormat'
import type { FeedEntry } from './timeline'

export interface ItemUsage {
  costUsd: number
  workMs: number
  turns: number
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
    const work = finitePositive(entry.activity?.thinkingMs)
    if (cost === 0 && work === 0) continue
    total.costUsd += cost
    total.workMs += work
    total.turns += 1
  }
  return total
}

const USAGE_SEPARATOR = ' · '

// Texto curto para o card ("$0.75 · 1m 5s"); sem turnos medidos não há o que mostrar.
export function formatItemUsage(usage: ItemUsage): string | null {
  if (usage.turns === 0) return null
  const parts = [formatCostUsd(usage.costUsd), formatThinkingDuration(usage.workMs)].filter((part): part is string => part !== null)
  return parts.length > 0 ? parts.join(USAGE_SEPARATOR) : null
}
