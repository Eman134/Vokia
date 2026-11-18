import { ref } from 'vue'
import { defineStore } from 'pinia'
import { useFeed } from '../team/useFeed'
import {
  discardInterruptedRun,
  listInterruptedRuns,
  resumeInterruptedRun,
  type InterruptedRun,
} from '../team/teamApi'
import { usePanelTab } from './usePanelTab'

export const useInterruptedRuns = defineStore('interruptedRuns', () => {
  const runs = ref<InterruptedRun[]>([])
  const feed = useFeed()
  const panelTab = usePanelTab()

  function reportError(error: unknown): void {
    feed.feed.push({ type: 'error', message: String(error) })
  }

  async function load(): Promise<void> {
    try {
      runs.value = await listInterruptedRuns()
    } catch (error) {
      reportError(error)
    }
  }

  // Só na carga do app: abre o painel na aba Agora quando há interrompidos, sem reabri-lo depois que o usuário fechar.
  async function start(): Promise<void> {
    await load()
    if (runs.value.length > 0) {
      panelTab.openOn('agora')
    }
  }

  function removeLocal(id: number): void {
    runs.value = runs.value.filter((run) => run.id !== id)
  }

  // O item sai da lista já no clique: a retomada só termina com o fluxo inteiro. Se falhar, o servidor devolve o pedido à lista.
  async function resume(run: InterruptedRun): Promise<void> {
    removeLocal(run.id)
    try {
      await resumeInterruptedRun(run.id)
    } catch (error) {
      reportError(error)
      await load()
    }
  }

  async function discard(run: InterruptedRun): Promise<void> {
    try {
      await discardInterruptedRun(run.id)
      removeLocal(run.id)
    } catch (error) {
      reportError(error)
      await load()
    }
  }

  return { runs, start, resume, discard }
})
