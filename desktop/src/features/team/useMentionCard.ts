import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { Agent } from './events'
import { useRequests } from './useRequests'

const CLOSE_DELAY_MS = 150

export interface MentionAnchor {
  left: number
  top: number
  bottom: number
}

export const useMentionCard = defineStore('mentionCard', () => {
  const requests = useRequests()
  const agent = ref<Agent | null>(null)
  const anchor = ref<MentionAnchor | null>(null)
  let closeTimer: ReturnType<typeof setTimeout> | null = null

  function cancelClose(): void {
    if (closeTimer !== null) clearTimeout(closeTimer)
    closeTimer = null
  }

  function open(target: Agent, element: Element): void {
    cancelClose()
    const { left, top, bottom } = element.getBoundingClientRect()
    agent.value = target
    anchor.value = { left, top, bottom }
  }

  function close(): void {
    cancelClose()
    agent.value = null
    anchor.value = null
  }

  // O atraso deixa o ponteiro ir da menção até o cartão sem fechá-lo no meio do caminho.
  function scheduleClose(): void {
    cancelClose()
    closeTimer = setTimeout(close, CLOSE_DELAY_MS)
  }

  function insert(target: Agent): void {
    requests.requestMention(target)
    close()
  }

  return { agent, anchor, open, close, scheduleClose, cancelClose, insert }
})
