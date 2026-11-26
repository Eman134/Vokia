import { computed, onScopeDispose, ref, watch, type ComputedRef } from 'vue'
import { useProjects } from '../projects/useProjects'
import { activeThinkingMsForItem, EMPTY_ITEM_USAGE, itemUsageIn, type ItemUsage } from './itemUsage'
import { useFeed } from './useFeed'

const CLOCK_TICK_MS = 1_000
const nowTick = ref(Date.now())
const activeTokens = new Set<symbol>()
let clockTimer: ReturnType<typeof setInterval> | null = null

function retainClock(token: symbol): void {
  activeTokens.add(token)
  if (clockTimer !== null) return
  nowTick.value = Date.now()
  clockTimer = setInterval(() => {
    nowTick.value = Date.now()
  }, CLOCK_TICK_MS)
}

function releaseClock(token: symbol): void {
  if (!activeTokens.delete(token) || activeTokens.size > 0 || clockTimer === null) return
  clearInterval(clockTimer)
  clockTimer = null
}

// Reativo: soma turnos concluídos do feed e o tempo pensando ao vivo no card usando um único relógio compartilhado sob demanda.
export function useItemUsage(itemId: () => string | undefined): ComputedRef<ItemUsage> {
  const feed = useFeed()
  const projects = useProjects()
  const token = Symbol()

  const isThinkingOnItem = computed(() => {
    const id = itemId()
    const projectId = projects.selected?.id
    if (id === undefined || projectId === undefined) return false
    return Object.entries(feed.states).some(
      ([agent, state]) => state === 'thinking' && feed.stateItem[agent] === id && feed.lastProject[agent] === projectId,
    )
  })

  watch(
    isThinkingOnItem,
    (active) => {
      if (active) retainClock(token)
      else releaseClock(token)
    },
    { immediate: true },
  )

  onScopeDispose(() => releaseClock(token))

  return computed(() => {
    const id = itemId()
    if (id === undefined) return EMPTY_ITEM_USAGE
    const base = itemUsageIn(feed.feed, id)
    if (!isThinkingOnItem.value) return base
    const activeMs = activeThinkingMsForItem({
      itemId: id,
      projectId: projects.selected?.id,
      states: feed.states,
      stateSince: feed.stateSince,
      stateItem: feed.stateItem,
      lastProject: feed.lastProject,
      now: nowTick.value,
    })
    return activeMs > 0 ? { ...base, workMs: base.workMs + activeMs } : base
  })
}
