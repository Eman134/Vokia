import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import {
  getUsageAnalytics,
  type UsageAnalyticsQuery,
  type UsageAnalyticsResponse,
} from './usageAnalyticsApi'

export type PeriodPreset = '7' | '14' | '30' | '90' | 'custom'

export const PERIOD_OPTIONS: readonly { label: string; value: PeriodPreset }[] = [
  { label: '7 d', value: '7' },
  { label: '14 d', value: '14' },
  { label: '30 d', value: '30' },
  { label: '90 d', value: '90' },
  { label: 'Personalizado', value: 'custom' },
]

export const UNPRICED_COST_LABEL = 'Sem estimativa'
const CENT = 0.01

function localIsoDate(date: Date): string {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function defaultCustomBounds(): { from: string; to: string } {
  const to = new Date()
  const from = new Date(to)
  from.setDate(to.getDate() - 29)
  return { from: localIsoDate(from), to: localIsoDate(to) }
}

function detectTimezone(): string {
  try {
    const tz = Intl.DateTimeFormat().resolvedOptions().timeZone
    if (tz && tz.trim().length > 0) return tz
  } catch {
    // Fallback para offset numérico abaixo.
  }
  const offsetMinutes = -new Date().getTimezoneOffset()
  const sign = offsetMinutes < 0 ? '-' : '+'
  const abs = Math.abs(offsetMinutes)
  const hh = String(Math.floor(abs / 60)).padStart(2, '0')
  const mm = String(abs % 60).padStart(2, '0')
  return `${sign}${hh}:${mm}`
}

export function formatEstimatedUsd(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return UNPRICED_COST_LABEL
  if (value > 0 && value < CENT) return `$${value.toFixed(4)}`
  return `$${value.toFixed(2)}`
}

export function formatDurationMs(durationMs: number): string {
  if (!Number.isFinite(durationMs) || durationMs <= 0) return '0s'
  const totalSeconds = Math.round(durationMs / 1000)
  if (totalSeconds < 60) return `${totalSeconds}s`
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  if (minutes < 60) return seconds === 0 ? `${minutes}m` : `${minutes}m ${seconds}s`
  const hours = Math.floor(minutes / 60)
  const remMinutes = minutes % 60
  return remMinutes === 0 ? `${hours}h` : `${hours}h ${remMinutes}m`
}

export const useUsageAnalytics = defineStore('usage-analytics', () => {
  const initialBounds = defaultCustomBounds()
  const period = ref<PeriodPreset>('30')
  const customFrom = ref<string>(initialBounds.from)
  const customTo = ref<string>(initialBounds.to)
  const timezone = ref<string>(detectTimezone())
  const projectId = ref<number | null>(null)
  const agentId = ref<string | null>(null)
  const providerId = ref<string | null>(null)

  const data = ref<UsageAnalyticsResponse | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)
  let requestSeq = 0

  const isEmpty = computed(() => data.value !== null && data.value.totals.turns === 0)

  function buildQuery(): UsageAnalyticsQuery {
    const query: UsageAnalyticsQuery = {
      timezone: timezone.value,
    }
    if (period.value === 'custom') {
      if (customFrom.value.trim()) query.from = customFrom.value.trim()
      if (customTo.value.trim()) query.to = customTo.value.trim()
    } else {
      query.days = Number(period.value)
    }
    if (projectId.value !== null) query.projectId = projectId.value
    if (agentId.value !== null && agentId.value.trim()) query.agentId = agentId.value.trim()
    if (providerId.value !== null && providerId.value.trim()) query.providerId = providerId.value.trim()
    return query
  }

  async function load(): Promise<void> {
    const seq = ++requestSeq
    loading.value = true
    error.value = null
    try {
      const response = await getUsageAnalytics(buildQuery())
      if (seq !== requestSeq) return
      data.value = response
    } catch (err) {
      if (seq !== requestSeq) return
      error.value = String(err)
    } finally {
      if (seq === requestSeq) {
        loading.value = false
      }
    }
  }

  async function setPeriod(next: PeriodPreset): Promise<void> {
    period.value = next
    if (next !== 'custom') {
      await load()
    }
  }

  async function applyCustomRange(from: string, to: string): Promise<void> {
    customFrom.value = from
    customTo.value = to
    period.value = 'custom'
    await load()
  }

  async function setProjectFilter(next: number | null): Promise<void> {
    projectId.value = next
    await load()
  }

  async function setAgentFilter(next: string | null): Promise<void> {
    agentId.value = next
    await load()
  }

  async function setProviderFilter(next: string | null): Promise<void> {
    providerId.value = next
    await load()
  }

  return {
    period,
    customFrom,
    customTo,
    timezone,
    projectId,
    agentId,
    providerId,
    data,
    loading,
    error,
    isEmpty,
    load,
    setPeriod,
    applyCustomRange,
    setProjectFilter,
    setAgentFilter,
    setProviderFilter,
  }
})
