import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { Agent } from './events'
import { useFeed } from './useFeed'

export type ConversationTab = 'conversa' | 'terminal' | 'fila'

export const useConversationTab = defineStore('conversationTab', () => {
  const tab = ref<ConversationTab>('conversa')
  const feed = useFeed()

  function showTerminal(agent: Agent): void {
    feed.showScreen({ kind: 'agent', agent })
    tab.value = 'terminal'
  }

  function reset(): void {
    tab.value = 'conversa'
  }

  return { tab, showTerminal, reset }
})
