import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { ALERT_TIMEOUT_MS, CONFIRM_VISIBLE_MS, MOMENT_TIMEOUT_MS, type BalloonItem, type BalloonSource } from './balloonTypes'

export const useBalloons = defineStore('balloons', () => {
  const queue = ref<BalloonItem[]>([])
  const current = ref<BalloonItem | null>(null)
  const confirmation = ref<string | null>(null)
  const suggestions = ref<BalloonItem[]>([])
  const panelOpen = ref(false)
  let momentTimer: ReturnType<typeof setTimeout> | null = null
  let alertCount = 0
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

  // Alerta só avisa: some sozinho e não vira sugestão guardada.
  function onAlertTimeout(): void {
    momentTimer = null
    close()
  }

  watch(current, (item) => {
    cancelMomentTimer()
    if (panelOpen.value) {
      return
    }
    if (item?.kind === 'moment') {
      momentTimer = setTimeout(onMomentTimeout, MOMENT_TIMEOUT_MS)
    } else if (item?.kind === 'alert') {
      momentTimer = setTimeout(onAlertTimeout, ALERT_TIMEOUT_MS)
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

  // Aviso temporário de uma ação importante: sem botões, fecha em ALERT_TIMEOUT_MS ou com Esc.
  function alert(title: string, text?: string, source: BalloonSource = 'conversa'): void {
    alertCount += 1
    enqueue({ id: `alert:${alertCount}`, kind: 'alert', source, title, text, actions: [] })
  }

  async function act(actionId: string): Promise<void> {
    const item = current.value
    if (item === null || confirmation.value !== null) {
      return
    }
    const action = item.actions.find((candidate) => candidate.id === actionId)
    if (action === undefined || action.disabled) {
      return
    }
    cancelMomentTimer()
    const text = await action.run?.()
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

  function replace(id: string, item: BalloonItem): boolean {
    if (current.value?.id === id) {
      current.value = item
      return true
    }
    const index = queue.value.findIndex((candidate) => candidate.id === id)
    if (index === -1) {
      return false
    }
    queue.value.splice(index, 1, item)
    return true
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
    alert,
    replace,
    act,
    dismiss,
    setPanelOpen,
  }
})
