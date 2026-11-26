import { ref } from 'vue'
import { defineStore } from 'pinia'
import {
  ALERT_TIMEOUT_MS,
  CONFIRM_VISIBLE_MS,
  MOMENT_TIMEOUT_MS,
  REQUEST_TIMEOUT_MS,
  type BalloonItem,
  type BalloonKind,
  type BalloonSource,
} from './balloonTypes'

const DISMISS_LABEL = 'Dispensar'

const KIND_TIMEOUT_MS: Partial<Record<BalloonKind, number>> = {
  moment: MOMENT_TIMEOUT_MS,
  alert: ALERT_TIMEOUT_MS,
  request: REQUEST_TIMEOUT_MS,
}

export const useBalloons = defineStore('balloons', () => {
  const items = ref<BalloonItem[]>([])
  const confirmations = ref<Record<string, string>>({})
  const suggestions = ref<BalloonItem[]>([])
  const panelOpen = ref(false)
  const autoTimers = new Map<string, ReturnType<typeof setTimeout>>()
  const confirmTimers = new Map<string, ReturnType<typeof setTimeout>>()
  const hoveredIds = new Set<string>()
  let alertCount = 0

  function clearAutoTimer(id: string): void {
    const timer = autoTimers.get(id)
    if (timer !== undefined) {
      clearTimeout(timer)
      autoTimers.delete(id)
    }
  }

  function clearConfirmTimer(id: string): void {
    const timer = confirmTimers.get(id)
    if (timer !== undefined) {
      clearTimeout(timer)
      confirmTimers.delete(id)
    }
  }

  function clearAllTimers(): void {
    for (const timer of autoTimers.values()) {
      clearTimeout(timer)
    }
    autoTimers.clear()
    for (const timer of confirmTimers.values()) {
      clearTimeout(timer)
    }
    confirmTimers.clear()
  }

  function removeConfirmation(id: string): void {
    if (confirmations.value[id] === undefined) {
      return
    }
    const next = { ...confirmations.value }
    delete next[id]
    confirmations.value = next
  }

  function onTimeout(id: string): void {
    autoTimers.delete(id)
    const item = items.value.find((candidate) => candidate.id === id)
    if (item?.kind === 'moment') {
      suggestions.value.push(item)
    }
    dismiss(id)
  }

  function startTimer(item: BalloonItem): void {
    clearAutoTimer(item.id)
    if (panelOpen.value || hoveredIds.has(item.id) || confirmations.value[item.id] !== undefined) {
      return
    }
    const timeoutMs = KIND_TIMEOUT_MS[item.kind]
    if (timeoutMs === undefined) {
      return
    }
    autoTimers.set(item.id, setTimeout(() => onTimeout(item.id), timeoutMs))
  }

  // O mouse sobre o balão congela o tempo dele; ao sair, a contagem recomeça do zero.
  function setHovered(id: string, value: boolean): void {
    if (value) {
      hoveredIds.add(id)
      clearAutoTimer(id)
      return
    }
    hoveredIds.delete(id)
    const item = items.value.find((candidate) => candidate.id === id)
    if (item !== undefined) {
      startTimer(item)
    }
  }

  function enqueue(item: BalloonItem): void {
    if (items.value.some((candidate) => candidate.id === item.id)) {
      return
    }
    items.value.push(item)
    startTimer(item)
  }

  // Aviso temporário de uma ação importante: fecha em ALERT_TIMEOUT_MS, com Esc ou no botão Dispensar.
  function alert(title: string, text?: string, source: BalloonSource = 'conversa'): void {
    alertCount += 1
    enqueue({
      id: `alert:${alertCount}`,
      kind: 'alert',
      source,
      title,
      text,
      actions: [{ id: 'dismiss', label: DISMISS_LABEL }],
    })
  }

  async function act(id: string, actionId: string): Promise<void> {
    const item = items.value.find((candidate) => candidate.id === id)
    if (item === undefined || confirmations.value[id] !== undefined) {
      return
    }
    const action = item.actions.find((candidate) => candidate.id === actionId)
    if (action === undefined || action.disabled) {
      return
    }
    clearAutoTimer(id)
    const text = await action.run?.()
    if (!items.value.some((candidate) => candidate.id === id)) {
      return
    }
    if (!text) {
      dismiss(id)
      return
    }
    confirmations.value = { ...confirmations.value, [id]: `✓ ${text}` }
    confirmTimers.set(id, setTimeout(() => dismiss(id), CONFIRM_VISIBLE_MS))
  }

  function dismiss(id: string): void {
    clearAutoTimer(id)
    clearConfirmTimer(id)
    hoveredIds.delete(id)
    removeConfirmation(id)
    items.value = items.value.filter((candidate) => candidate.id !== id)
  }

  function dismissAll(): void {
    clearAllTimers()
    hoveredIds.clear()
    confirmations.value = {}
    items.value = []
  }

  function replace(id: string, item: BalloonItem): boolean {
    const index = items.value.findIndex((candidate) => candidate.id === id)
    if (index === -1) {
      return false
    }
    if (item.id !== id) {
      clearAutoTimer(id)
      clearConfirmTimer(id)
      if (hoveredIds.delete(id)) {
        hoveredIds.add(item.id)
      }
      removeConfirmation(id)
    }
    items.value.splice(index, 1, item)
    startTimer(item)
    return true
  }

  function setPanelOpen(open: boolean): void {
    panelOpen.value = open
    if (open) {
      clearAllTimers()
      // Popups temporários (alert, moment, request) são descartados ao abrir o painel e não voltam depois.
      const remaining: BalloonItem[] = []
      for (const candidate of items.value) {
        if (candidate.kind === 'alert' || candidate.kind === 'moment' || candidate.kind === 'request') {
          hoveredIds.delete(candidate.id)
          removeConfirmation(candidate.id)
        } else {
          remaining.push(candidate)
        }
      }
      items.value = remaining
      const confirmingIds = Object.keys(confirmations.value)
      if (confirmingIds.length > 0) {
        for (const id of confirmingIds) {
          hoveredIds.delete(id)
        }
        confirmations.value = {}
        items.value = items.value.filter((candidate) => !confirmingIds.includes(candidate.id))
      }
      return
    }
    for (const item of items.value) {
      startTimer(item)
    }
  }

  return {
    items,
    confirmations,
    suggestions,
    panelOpen,
    enqueue,
    alert,
    replace,
    act,
    dismiss,
    dismissAll,
    setPanelOpen,
    setHovered,
  }
})
