import type { TurnUsage } from './events'

const THOUSAND = 1_000
const MILLION = 1_000_000
const CENT = 0.01
const MS_PER_SECOND = 1_000
const SECONDS_PER_MINUTE = 60
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

export function formatUsageModel(usage?: TurnUsage): string | null {
  return usage?.model?.trim() || null
}

export function formatThinkingDuration(thinkingMs?: number): string | null {
  if (thinkingMs === undefined || !Number.isFinite(thinkingMs) || thinkingMs < 0) {
    return null
  }
  const totalSeconds = Math.floor(Math.max(thinkingMs, MS_PER_SECOND) / MS_PER_SECOND)
  if (totalSeconds < SECONDS_PER_MINUTE) {
    return `${totalSeconds}s`
  }
  const minutes = Math.floor(totalSeconds / SECONDS_PER_MINUTE)
  const seconds = totalSeconds % SECONDS_PER_MINUTE
  return seconds === 0 ? `${minutes}m` : `${minutes}m ${seconds}s`
}

export type TokenSide = 'in' | 'out'

export interface TokenSplit {
  inputDegrees: number
  outputDegrees: number
  dominant: TokenSide
  label: string
}

const FULL_TURN_DEGREES = 360
// Garante que o lado minoritário (ex.: 2 in vs 864 out) continue visível no anel.
const MIN_SLICE_DEGREES = 20
const PERCENT = 100
const NEAR_FULL_PERCENT = 99

export function tokenSplit(usage?: TurnUsage): TokenSplit | null {
  const input = Math.max(usage?.inputTokens ?? 0, 0)
  const output = Math.max(usage?.outputTokens ?? 0, 0)
  const total = input + output
  if (!Number.isFinite(total) || total <= 0) return null

  const dominant: TokenSide = output > input ? 'out' : 'in'
  const dominantTokens = dominant === 'out' ? output : input
  const minorTokens = total - dominantTokens
  const minorDegrees =
    minorTokens === 0 ? 0 : Math.max((minorTokens / total) * FULL_TURN_DEGREES, MIN_SLICE_DEGREES)
  const dominantDegrees = FULL_TURN_DEGREES - minorDegrees

  const percent = (dominantTokens / total) * PERCENT
  const rounded = Math.round(percent)
  const shown = minorTokens === 0 ? '100%' : rounded > NEAR_FULL_PERCENT ? `>${NEAR_FULL_PERCENT}%` : `${rounded}%`

  return {
    inputDegrees: dominant === 'in' ? dominantDegrees : minorDegrees,
    outputDegrees: dominant === 'out' ? dominantDegrees : minorDegrees,
    dominant,
    label: `${shown} ${dominant}`,
  }
}

export interface UsageDetail {
  label: string
  value: string
}

export function formatUsageDetails(usage?: TurnUsage): UsageDetail[] {
  if (!usage) return []
  const details: UsageDetail[] = []
  const model = formatUsageModel(usage)
  if (model) details.push({ label: 'Modelo', value: model })
  if (usage.inputTokens != null) details.push({ label: 'Entrada', value: `${INT_FORMAT.format(usage.inputTokens)} tokens` })
  if (usage.outputTokens != null) details.push({ label: 'Saída', value: `${INT_FORMAT.format(usage.outputTokens)} tokens` })
  if (usage.cachedInputTokens != null && usage.cachedInputTokens > 0) {
    details.push({ label: 'Cache (leitura)', value: `${INT_FORMAT.format(usage.cachedInputTokens)} tokens` })
  }
  if (usage.inputTokens != null || usage.outputTokens != null) {
    const total = (usage.inputTokens ?? 0) + (usage.outputTokens ?? 0)
    details.push({ label: 'Total', value: `${INT_FORMAT.format(total)} tokens` })
  }
  const cost = formatCostUsd(usage.costUsd)
  if (cost !== null) details.push({ label: 'Custo', value: cost })
  return details
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
