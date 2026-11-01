import { ref } from 'vue'
import { defineStore } from 'pinia'
import { useDecisions } from '../decisions/useDecisions'

export type PanelTab = 'decisoes' | 'agora'

export const usePanelTab = defineStore('panelTab', () => {
  const tab = ref<PanelTab>('decisoes')

  function openOn(next: PanelTab): void {
    tab.value = next
    useDecisions().setOpen(true)
  }

  return { tab, openOn }
})
