import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { useProviders } from '../providers/useProviders'
import { useActiveLlm } from '../providers/useActiveLlm'
import type {
  PlanUsageSnapshot,
  ProviderPlanUsage,
  ProviderUsageWindow,
  UsageLimitEntry,
  UsageWindow,
  VokiaPlanUsage,
} from '../team/events'
import {
  DEFAULT_THRESHOLDS,
  FIVE_HOUR_MINS,
  MS_PER_SECOND,
  WEEKLY_MINS,
  allWindowsOf,
  isUsingCredits,
  mostRestrictiveWindow,
  providerShortName,
  type ProviderThresholds,
} from './usagePlans'
import { getPlanUsage, listenUsageUpdates, refreshPlanUsage, type UnlistenFn } from './usageApi'

const CLOCK_INTERVAL_MS = 60_000
const POLL_INTERVAL_MS = 5 * 60_000

interface LegacyClaudeUsage {
  fiveHour: UsageWindow
  sevenDay: UsageWindow
  updatedAt: number
}

export interface CriticalUsageSummary {
  provider: ProviderPlanUsage
  window: ProviderUsageWindow
  usingCredits: boolean
  thresholds: ProviderThresholds
}

export const useUsage = defineStore('usage', () => {
  const providers = ref<Record<string, ProviderPlanUsage>>({})
  const activeLimits = ref<UsageLimitEntry[]>([])
  const thresholdsByProvider = ref<Record<string, ProviderThresholds>>({})
  const now = ref(Date.now())
  const startupError = ref<string | null>(null)
  const refreshing = ref(false)

  let running = false
  let unlisten: UnlistenFn | null = null
  let clockTimer: ReturnType<typeof setInterval> | null = null
  let pollTimer: ReturnType<typeof setInterval> | null = null

  function thresholdsOf(providerId: string): ProviderThresholds {
    return thresholdsByProvider.value[providerId] ?? DEFAULT_THRESHOLDS
  }

  function setProviderThresholds(providerId: string, thresholds: Partial<ProviderThresholds>): void {
    const current = thresholdsOf(providerId)
    thresholdsByProvider.value = {
      ...thresholdsByProvider.value,
      [providerId]: {
        warn: thresholds.warn ?? current.warn,
        pause: thresholds.pause ?? current.pause,
      },
    }
  }

  const providersStore = useProviders()
  const activeLlm = useActiveLlm()
  const disabledProviderIds = computed(() => {
    const disabled = new Set<string>()
    for (const entry of providersStore.providers) {
      if (!entry.enabled) disabled.add(entry.id)
    }
    return disabled
  })

  const providerList = computed<ProviderPlanUsage[]>(() =>
    Object.values(providers.value)
      .filter((entry) => !disabledProviderIds.value.has(entry.providerId))
      .sort((a, b) => a.providerId.localeCompare(b.providerId)),
  )

  const latestUpdatedAtMs = computed<number | null>(() => {
    let latest: number | null = null
    for (const entry of providerList.value) {
      if (entry.updatedAtMs > 0 && (latest === null || entry.updatedAtMs > latest)) {
        latest = entry.updatedAtMs
      }
    }
    return latest
  })

  const mostCritical = computed<CriticalUsageSummary | null>(() => {
    const candidates: CriticalUsageSummary[] = []
    for (const provider of providerList.value) {
      const window = mostRestrictiveWindow(provider, now.value)
      if (!window) continue
      candidates.push({
        provider,
        window,
        usingCredits: isUsingCredits(provider),
        thresholds: thresholdsOf(provider.providerId),
      })
    }
    if (candidates.length === 0) return null
    const primaryCli = activeLlm.active.cli.trim()
    if (primaryCli) {
      const primaryMatch = candidates.find((item) => item.provider.providerId === primaryCli)
      if (primaryMatch) return primaryMatch
    }
    if (activeLlm.fallback.enabled) {
      for (const entry of activeLlm.fallback.chain) {
        const fbMatch = candidates.find((item) => item.provider.providerId === entry.cli)
        if (fbMatch) return fbMatch
      }
    }
    const blocking = candidates.filter((item) => !item.usingCredits)
    const pool = blocking.length > 0 ? blocking : candidates
    return pool.reduce((worst, current) =>
      current.window.percent > worst.window.percent ? current : worst,
    )
  })

  // Compatibilidade com assinantes legados (useLimitNotices)
  const claude = computed<LegacyClaudeUsage | null>(() => {
    const entry = providers.value.claude
    if (!entry?.primary) return null
    return {
      fiveHour: { percent: entry.primary.percent, resetsAt: entry.primary.resetsAt },
      sevenDay: {
        percent: entry.secondary?.percent ?? 0,
        resetsAt: entry.secondary?.resetsAt ?? 0,
      },
      updatedAt: entry.updatedAtMs,
    }
  })

  const visible = computed(() => providerList.value.some((entry) => allWindowsOf(entry).length > 0))

  const fiveHour = computed<number | null>(() => {
    const critical = mostCritical.value
    if (!critical || critical.usingCredits) return null
    if (
      critical.window.resetsAt > 0 &&
      now.value >= critical.window.resetsAt * MS_PER_SECOND
    ) {
      return null
    }
    return critical.window.percent
  })

  const alert = computed(() => {
    const critical = mostCritical.value
    if (!critical || critical.usingCredits) return false
    return critical.window.percent >= critical.thresholds.warn
  })

  const openable = computed(() => providerList.value.length > 0 || startupError.value !== null)

  const ariaLabel = computed(() => {
    if (providerList.value.length === 0) {
      return 'Uso dos planos: sem dados'
    }
    const parts = providerList.value
      .map((provider) => {
        const window = mostRestrictiveWindow(provider, now.value)
        if (!window) return null
        const label = providerShortName(provider)
        if (isUsingCredits(provider)) {
          return `${label} usando créditos`
        }
        return `${label} ${window.percent}%`
      })
      .filter((item): item is string => item !== null)
    return parts.length > 0 ? `Uso dos planos: ${parts.join(', ')}` : 'Uso dos planos: sem dados'
  })

  function refreshClock(): void {
    now.value = Date.now()
  }

  function applyProviderUsage(usage: ProviderPlanUsage): void {
    refreshClock()
    providers.value = {
      ...providers.value,
      [usage.providerId]: usage,
    }
  }

  function applySnapshot(snapshot: PlanUsageSnapshot): void {
    refreshClock()
    const next: Record<string, ProviderPlanUsage> = { ...providers.value }
    for (const usage of snapshot.providers) {
      next[usage.providerId] = usage
    }
    providers.value = next
    activeLimits.value = Array.isArray(snapshot.activeLimits) ? snapshot.activeLimits : []
  }

  function apply(usage: VokiaPlanUsage): void {
    refreshClock()
    // Evita sobrescrever se já recebemos atualização rica recente do mesmo provedor
    const existing = providers.value.claude
    if (existing && now.value - existing.updatedAtMs < 2_000) {
      return
    }
    applyProviderUsage({
      providerId: 'claude',
      label: 'Claude Code',
      primary: {
        percent: usage.fiveHour.percent,
        resetsAt: usage.fiveHour.resetsAt,
        durationMins: FIVE_HOUR_MINS,
        kind: 'five-hour',
      },
      secondary: {
        percent: usage.sevenDay.percent,
        resetsAt: usage.sevenDay.resetsAt,
        durationMins: WEEKLY_MINS,
        kind: 'weekly',
      },
      updatedAtMs: now.value,
      readFailed: false,
    })
  }

  async function refresh(providerId?: string): Promise<void> {
    if (refreshing.value) return
    refreshing.value = true
    try {
      const snapshot = await refreshPlanUsage(providerId)
      applySnapshot(snapshot)
    } catch {
      // O backend mantém a última leitura válida em backoff; falhas silenciosas não derrubam o painel.
    } finally {
      refreshing.value = false
    }
  }

  async function start(): Promise<void> {
    if (running) return
    running = true
    if (providersStore.providers.length === 0) {
      void providersStore.load().catch(() => {})
    }
    try {
      const stopListening = await listenUsageUpdates({
        onProviderUsage: applyProviderUsage,
        onLegacyPlanUsage: apply,
      })
      if (!running || unlisten !== null) {
        stopListening()
        return
      }
      unlisten = stopListening
      clockTimer = setInterval(refreshClock, CLOCK_INTERVAL_MS)
      pollTimer = setInterval(() => {
        void refresh()
      }, POLL_INTERVAL_MS)
      try {
        const initial = await getPlanUsage()
        applySnapshot(initial)
      } catch {
        // Sem leituras iniciais disponíveis ainda
      }
    } catch (error) {
      running = false
      startupError.value = `Não foi possível iniciar o uso dos planos: ${String(error)}`
    }
  }

  function stop(): void {
    running = false
    unlisten?.()
    unlisten = null
    if (clockTimer !== null) {
      clearInterval(clockTimer)
      clockTimer = null
    }
    if (pollTimer !== null) {
      clearInterval(pollTimer)
      pollTimer = null
    }
  }

  return {
    providers,
    activeLimits,
    providerList,
    latestUpdatedAtMs,
    mostCritical,
    thresholdsByProvider,
    claude,
    now,
    visible,
    fiveHour,
    alert,
    ariaLabel,
    openable,
    startupError,
    refreshing,
    thresholdsOf,
    setProviderThresholds,
    apply,
    applyProviderUsage,
    applySnapshot,
    refresh,
    refreshClock,
    start,
    stop,
  }
})
