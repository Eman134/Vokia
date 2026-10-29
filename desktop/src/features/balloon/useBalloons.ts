import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { CONFIRM_VISIBLE_MS, MOMENT_TIMEOUT_MS, type BalloonItem } from './balloonTypes'

export const useBalloons = defineStore('balloons', () => {
  const queue = ref<BalloonItem[]>([])
  const current = ref<BalloonItem | null>(null)
  const confirmation = ref<string | null>(null)
  const suggestions = ref<BalloonItem[]>([])
  const panelOpen = ref(false)
  let momentTimer: ReturnType<typeof setTimeout> | null = null
  let confirmTimer: ReturnType<typeof setTimeout> | null = null

  function cancelMomentTimer(): void {
    if (momentTimer !== null) {
      clearTimeout(momentTimer)
      momentTimer = null
    }
  }

  function cancelConfirmTimer(): void {
    if (confirmTimer !== null) {
      clearTimeout(confirmTimer)
      confirmTimer = null
    }
  }

  function advance(): void {
    if (current.value !== null || panelOpen.value) {
      return
    }
    current.value = queue.value.shift() ?? null
  }

  function close(): void {
    cancelMomentTimer()
    cancelConfirmTimer()
    confirmation.value = null
    current.value = null
    advance()
  }

  function onMomentTimeout(): void {
    momentTimer = null
    if (current.value === null) {
      return
    }
    suggestions.value.push(current.value)
    close()
  }

  watch(current, (item) => {
    cancelMomentTimer()
    if (item?.kind === 'moment' && !panelOpen.value) {
      momentTimer = setTimeout(onMomentTimeout, MOMENT_TIMEOUT_MS)
    }
  })

  function isKnown(id: string): boolean {
    return current.value?.id === id || queue.value.some((item) => item.id === id)
  }

  function enqueue(item: BalloonItem): void {
    if (isKnown(item.id)) {
      return
    }
    queue.value.push(item)
    advance()
  }

  function act(actionId: string): void {
    const item = current.value
    if (item === null || confirmation.value !== null) {
      return
    }
    const action = item.actions.find((candidate) => candidate.id === actionId)
    if (action === undefined || action.disabled) {
      return
    }
    cancelMomentTimer()
    const text = action.run?.()
    if (!text) {
      close()
      return
    }
    confirmation.value = `✓ ${text}`
    confirmTimer = setTimeout(close, CONFIRM_VISIBLE_MS)
  }

  function dismiss(): void {
    close()
  }

  function setPanelOpen(open: boolean): void {
    panelOpen.value = open
    if (!open) {
      advance()
      return
    }
    cancelMomentTimer()
    // Confirmação em curso já executou a ação: não volta à fila, o timer só fecha o balão.
    if (current.value !== null && confirmation.value === null) {
      queue.value.unshift(current.value)
      current.value = null
    }
  }

  return {
    queue,
    current,
    confirmation,
    suggestions,
    panelOpen,
    enqueue,
    act,
    dismiss,
    setPanelOpen,
  }
})
