import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import type { UsageWindow, VokiaPlanUsage } from '../team/events'
import { MS_PER_SECOND, WARN_THRESHOLD } from './usagePlans'
import { listenPlanUsage, type UnlistenFn } from './usageApi'

const CLOCK_INTERVAL_MS = 60000

interface ClaudeUsage {
  fiveHour: UsageWindow
  sevenDay: UsageWindow
  updatedAt: number
}

export const useUsage = defineStore('usage', () => {
  const claude = ref<ClaudeUsage | null>(null)
  const now = ref(Date.now())
  const startupError = ref<string | null>(null)
  let running = false
  let unlisten: UnlistenFn | null = null
  let clockTimer: ReturnType<typeof setInterval> | null = null

  const visible = computed(
    () => claude.value !== null && now.value < claude.value.fiveHour.resetsAt * MS_PER_SECOND,
  )
  const fiveHour = computed(() =>
    claude.value !== null && visible.value ? claude.value.fiveHour.percent : null,
  )
  const alert = computed(() => fiveHour.value !== null && fiveHour.value >= WARN_THRESHOLD)
  const openable = computed(() => visible.value || startupError.value !== null)
  const ariaLabel = computed(() =>
    fiveHour.value === null
      ? 'Uso dos planos: sem dados'
      : `Uso dos planos: Claude ${fiveHour.value}% da janela de 5 h`,
  )

  function refreshClock(): void {
    now.value = Date.now()
  }

  function apply(usage: VokiaPlanUsage): void {
    refreshClock()
    claude.value = { fiveHour: usage.fiveHour, sevenDay: usage.sevenDay, updatedAt: now.value }
  }

  async function start(): Promise<void> {
    if (running) return
    running = true
    try {
      const stopListening = await listenPlanUsage(apply)
      // stop() or a concurrent start() may have run while listen was pending
      if (!running || unlisten !== null) {
        stopListening()
        return
      }
      unlisten = stopListening
      clockTimer = setInterval(refreshClock, CLOCK_INTERVAL_MS)
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
  }

  return {
    claude,
    now,
    visible,
    fiveHour,
    alert,
    ariaLabel,
    openable,
    startupError,
    apply,
    refreshClock,
    start,
    stop,
  }
})
