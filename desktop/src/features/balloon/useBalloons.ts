import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { ALERT_TIMEOUT_MS, CONFIRM_VISIBLE_MS, MOMENT_TIMEOUT_MS, REQUEST_TIMEOUT_MS, type BalloonItem, type BalloonSource } from './balloonTypes'

const DISMISS_LABEL = 'Dispensar'

// Avisos de conversa: não empilham, o mais novo substitui o anterior e conta quantos ele agrupa.
const isChatNotice = (item: BalloonItem): boolean => item.kind === 'alert' || item.kind === 'request'
const noticeCount = (item: BalloonItem): number => item.count ?? 1

export const useBalloons = defineStore('balloons', () => {
  const queue = ref<BalloonItem[]>([])
  const current = ref<BalloonItem | null>(null)
  const confirmation = ref<string | null>(null)
  const suggestions = ref<BalloonItem[]>([])
  const panelOpen = ref(false)
  let momentTimer: ReturnType<typeof setTimeout> | null = null
  let alertCount = 0
  let confirmTimer: ReturnType<typeof setTimeout> | null = null
  let hovered = false

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
    hovered = false
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

  function startTimer(item: BalloonItem | null): void {
    cancelMomentTimer()
    if (panelOpen.value || hovered || confirmation.value !== null) {
      return
    }
    if (item?.kind === 'moment') {
      momentTimer = setTimeout(onMomentTimeout, MOMENT_TIMEOUT_MS)
    } else if (item?.kind === 'alert') {
      momentTimer = setTimeout(onAlertTimeout, ALERT_TIMEOUT_MS)
    } else if (item?.kind === 'request') {
      momentTimer = setTimeout(onAlertTimeout, REQUEST_TIMEOUT_MS)
    }
  }

  watch(current, startTimer)

  // O mouse sobre o balão congela o tempo; ao sair, a contagem recomeça do zero.
  function setHovered(value: boolean): void {
    hovered = value
    if (value) {
      cancelMomentTimer()
    } else {
      startTimer(current.value)
    }
  }

  function isKnown(id: string): boolean {
    return current.value?.id === id || queue.value.some((item) => item.id === id)
  }

  function enqueue(item: BalloonItem): void {
    if (isKnown(item.id)) {
      return
    }
    if (isChatNotice(item)) {
      groupChatNotice(item)
      return
    }
    queue.value.push(item)
    advance()
  }

  function groupChatNotice(item: BalloonItem): void {
    const showing = current.value !== null && confirmation.value === null && isChatNotice(current.value) ? current.value : null
    const stale = queue.value.filter(isChatNotice)
    queue.value = queue.value.filter((queued) => !isChatNotice(queued))
    const count = [showing, ...stale].reduce((total, notice) => total + (notice ? noticeCount(notice) : 0), 1)
    const grouped = { ...item, count }
    if (showing) {
      current.value = grouped
      return
    }
    queue.value.push(grouped)
    advance()
  }

  // Aviso temporário de uma ação importante: fecha em ALERT_TIMEOUT_MS, com Esc ou no botão Dispensar.
  function alert(title: string, text?: string, source: BalloonSource = 'conversa'): void {
    alertCount += 1
    enqueue({ id: `alert:${alertCount}`, kind: 'alert', source, title, text, actions: [{ id: 'dismiss', label: DISMISS_LABEL }] })
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
      current.value = { ...item, count: current.value.count }
      return true
    }
    const index = queue.value.findIndex((candidate) => candidate.id === id)
    if (index === -1) {
      return false
    }
    queue.value.splice(index, 1, { ...item, count: queue.value[index]?.count })
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
    setHovered,
  }
})
