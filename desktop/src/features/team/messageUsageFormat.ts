import type { TurnUsage } from './events'

const THOUSAND = 1_000
const MILLION = 1_000_000
const CENT = 0.01
const INT_FORMAT = new Intl.NumberFormat('pt-BR')

export function formatTokenCount(value: number): string {
  if (!Number.isFinite(value) || value < 0) return '0'
  if (value < THOUSAND) return String(Math.round(value))
  if (value < MILLION) {
    return `${(value / THOUSAND).toFixed(1).replace(/\.0$/, '')}k`
  }
  return `${(value / MILLION).toFixed(1).replace(/\.0$/, '')}M`
}

export function formatCostUsd(value?: number): string | null {
  if (value === undefined || !Number.isFinite(value) || value <= 0) {
    return null
  }
  if (value < CENT) {
    return `$${value.toFixed(4)}`
  }
  return `$${value.toFixed(2)}`
}

export function formatUsageInline(usage?: TurnUsage): string | null {
  if (!usage) return null
  const parts: string[] = []
  const model = usage.model?.trim()
  if (model) {
    parts.push(model)
  }
  if (usage.inputTokens != null || usage.outputTokens != null) {
    const input = formatTokenCount(usage.inputTokens ?? 0)
    const output = formatTokenCount(usage.outputTokens ?? 0)
    parts.push(`${input} in · ${output} out`)
  }
  const cost = formatCostUsd(usage.costUsd)
  if (cost !== null) {
    parts.push(cost)
  }
  return parts.length > 0 ? parts.join(' · ') : null
}

export function formatUsageTooltip(usage?: TurnUsage): string {
  if (!usage) return ''
  const parts: string[] = []
  if (usage.provider?.trim()) {
    parts.push(`Provedor: ${usage.provider.trim()}`)
  }
  if (usage.model?.trim()) {
    parts.push(`Modelo: ${usage.model.trim()}`)
  }
  if (usage.inputTokens != null) {
    parts.push(`Entrada: ${INT_FORMAT.format(usage.inputTokens)} tokens`)
  }
  if (usage.cachedInputTokens != null && usage.cachedInputTokens > 0) {
    parts.push(`Cache: ${INT_FORMAT.format(usage.cachedInputTokens)} tokens`)
  }
  if (usage.outputTokens != null) {
    parts.push(`Saída: ${INT_FORMAT.format(usage.outputTokens)} tokens`)
  }
  const cost = formatCostUsd(usage.costUsd)
  if (cost !== null) {
    parts.push(`Custo: ${cost}`)
  }
  return parts.join(' · ')
}
