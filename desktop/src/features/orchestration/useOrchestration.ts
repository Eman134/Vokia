import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { useProjects } from '../projects/useProjects'
import { useFeed } from '../team/useFeed'
import type { VokiaBatchProgress } from '../team/events'
import * as orchestrationApi from './orchestrationApi'
import type { OrchPlacementDraft, OrchTeamDraft, OrchestrationSnapshot } from './orchestrationTypes'

export const useOrchestration = defineStore('orchestration', () => {
  const snapshot = ref<OrchestrationSnapshot | null>(null)
  const loadedProjectId = ref<number | null>(null)
  // Placar de cada lote aberto, por projeto e líder; o evento done tira o lote da lista.
  const batches = ref(new Map<string, VokiaBatchProgress>())

  function applyBatchProgress(event: VokiaBatchProgress): void {
    const key = `${event.projectId}:${event.leader}`
    const next = new Map(batches.value)
    if (event.done) {
      next.delete(key)
    } else {
      next.set(key, event)
    }
    batches.value = next
  }
  const feed = useFeed()
  const projects = useProjects()
  // Projeto da carga em curso ou da última: o evento que chega durante a primeira carga também recarrega.
  let requestedProjectId: number | null = null
  let loadSeq = 0

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error)
  }

  function reportError(error: unknown): void {
    feed.feed.push({ type: 'error', message: errorMessage(error) })
  }

  function resetLoaded(): void {
    snapshot.value = null
    loadedProjectId.value = null
  }

  async function fetchAll(projectId: number, seq: number): Promise<boolean> {
    try {
      const nextSnapshot = await orchestrationApi.fetchOrchestration(projectId)
      if (seq !== loadSeq) return false
      snapshot.value = nextSnapshot
      loadedProjectId.value = projectId
      return true
    } catch (error) {
      if (seq === loadSeq) reportError(error)
      return false
    }
  }

  function load(projectId: number): Promise<boolean> {
    const seq = (loadSeq += 1)
    requestedProjectId = projectId
    resetLoaded()
    return fetchAll(projectId, seq)
  }

  function clear(): void {
    loadSeq += 1
    requestedProjectId = null
    resetLoaded()
  }

  // Recarrega sem limpar a tela; resposta de carga anterior é descartada pela sequência.
  async function reload(projectId: number): Promise<void> {
    if (requestedProjectId !== projectId) return
    await fetchAll(projectId, (loadSeq += 1))
  }

  // Escrita não mexe no estado: a tela muda quando o evento orchestration-changed recarregar.
  async function write(operation: (projectId: number) => Promise<unknown>): Promise<boolean> {
    const projectId = loadedProjectId.value
    if (projectId === null) return false
    try {
      await operation(projectId)
      return true
    } catch (error) {
      reportError(error)
      return false
    }
  }

  function placeAgent(draft: OrchPlacementDraft): Promise<boolean> {
    return write((projectId) => orchestrationApi.placeAgent(projectId, draft))
  }

  function saveTeam(draft: OrchTeamDraft): Promise<boolean> {
    return write((projectId) => orchestrationApi.saveTeam(projectId, draft))
  }

  function deleteTeam(id: string): Promise<boolean> {
    return write((projectId) => orchestrationApi.deleteTeam(projectId, id))
  }

  watch(
    () => projects.selected?.id,
    (projectId) => {
      if (projectId === undefined) {
        clear()
        return
      }
      void load(projectId)
    },
    { immediate: true },
  )

  return {
    snapshot,
    loadedProjectId,
    batches,
    applyBatchProgress,
    clear,
    load,
    reload,
    placeAgent,
    saveTeam,
    deleteTeam,
  }
})
