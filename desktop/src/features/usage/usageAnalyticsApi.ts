import { invoke } from '@tauri-apps/api/core'

const GET_USAGE_ANALYTICS_COMMAND = 'get_usage_analytics'

export interface UsageAnalyticsQuery {
  days?: number
  from?: string
  to?: string
  timezone?: string
  projectId?: number
  agentId?: string
  providerId?: string
}

export interface UsageRange {
  from: string
  to: string
  days: number
  timezone: string
  offset: string
}

export interface UsageTotals {
  turns: number
  inputTokens: number
  cachedInputTokens: number
  cacheWriteTokens: number
  outputTokens: number
  thoughtTokens: number
  totalTokens: number
  costUsdEstimated: number | null
  pricedTurns: number
  unpricedTurns: number
  fallbackTurns: number
  durationMs: number
}

export interface UsageSavings {
  cacheSavingsUsd: number
  lightTierSavingsUsd: number
  totalSavingsUsd: number
  savingsPercent: number
  cacheHitPercent: number
}

export interface DailyProviderUsage {
  date: string
  providerId: string
  turns: number
  inputTokens: number
  cachedInputTokens: number
  cacheWriteTokens: number
  outputTokens: number
  thoughtTokens: number
  totalTokens: number
  costUsdEstimated: number | null
  unpricedTurns: number
  durationMs: number
}

export interface DailyUsage {
  date: string
  turns: number
  inputTokens: number
  cachedInputTokens: number
  cacheWriteTokens: number
  outputTokens: number
  thoughtTokens: number
  totalTokens: number
  costUsdEstimated: number | null
  unpricedTurns: number
  durationMs: number
}

export interface ProviderBreakdown {
  providerId: string
  turns: number
  inputTokens: number
  cachedInputTokens: number
  cacheWriteTokens: number
  outputTokens: number
  thoughtTokens: number
  totalTokens: number
  costUsdEstimated: number | null
  unpricedTurns: number
  fallbackTurns: number
  durationMs: number
}

export interface AgentBreakdown {
  agentId: string
  turns: number
  inputTokens: number
  cachedInputTokens: number
  cacheWriteTokens: number
  outputTokens: number
  thoughtTokens: number
  totalTokens: number
  costUsdEstimated: number | null
  unpricedTurns: number
  fallbackTurns: number
  durationMs: number
}

export interface ModelBreakdown {
  providerId: string
  model: string
  turns: number
  inputTokens: number
  cachedInputTokens: number
  cacheWriteTokens: number
  outputTokens: number
  thoughtTokens: number
  totalTokens: number
  costUsdEstimated: number | null
  unpricedTurns: number
  priceBasis: string | null
  durationMs: number
}

export interface TaskBreakdown {
  cardId: string
  projectId: number | null
  title: string | null
  publicId: string | null
  turns: number
  inputTokens: number
  cachedInputTokens: number
  cacheWriteTokens: number
  outputTokens: number
  thoughtTokens: number
  totalTokens: number
  costUsdEstimated: number | null
  unpricedTurns: number
  durationMs: number
  tokensPerSec: number | null
}

export interface UsageAnalyticsResponse {
  range: UsageRange
  totals: UsageTotals
  savings: UsageSavings
  dailyByProvider: DailyProviderUsage[]
  daily: DailyUsage[]
  byProvider: ProviderBreakdown[]
  byAgent: AgentBreakdown[]
  byModel: ModelBreakdown[]
  byTask: TaskBreakdown[]
  pricingVersion: string
  costDisclaimer: string
}

export function getUsageAnalytics(query: UsageAnalyticsQuery = {}): Promise<UsageAnalyticsResponse> {
  const payload: Record<string, unknown> = {}
  if (query.days !== undefined) payload.days = query.days
  if (query.from !== undefined) payload.from = query.from
  if (query.to !== undefined) payload.to = query.to
  if (query.timezone !== undefined) payload.timezone = query.timezone
  if (query.projectId !== undefined) payload.projectId = query.projectId
  if (query.agentId !== undefined) payload.agentId = query.agentId
  if (query.providerId !== undefined) payload.providerId = query.providerId
  return invoke<UsageAnalyticsResponse>(GET_USAGE_ANALYTICS_COMMAND, payload)
}
