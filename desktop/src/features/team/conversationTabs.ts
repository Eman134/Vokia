import { ref } from 'vue'
import { defineStore } from 'pinia'

export type ConversationTab = 'conversa' | 'fila'

export const useConversationTab = defineStore('conversationTab', () => {
  const tab = ref<ConversationTab>('conversa')

  function reset(): void {
    tab.value = 'conversa'
  }

  return { tab, reset }
})
