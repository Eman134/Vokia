import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { useProjects } from '../projects/useProjects'
import { useFeed } from '../team/useFeed'
import * as orchestrationApi from './orchestrationApi'
import type {
  OrchLayerDraft,
  OrchMovement,
  OrchPlacementDraft,
  OrchRuleDraft,
  OrchTeamDraft,
  OrchestrationSettings,
  OrchestrationSnapshot,
} from './orchestrationTypes'

export const useOrchestration = defineStore('orchestration', () => {
  const snapshot = ref<OrchestrationSnapshot | null>(null)
  const movements = ref<OrchMovement[]>([])
  const loadedProjectId = ref<number | null>(null)
  const loading = ref(false)
  const loadError = ref<string | null>(null)
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
    movements.value = []
    loadedProjectId.value = null
  }

  // Snapshot e movimentos entram juntos: se um dos dois falhar, nenhum entra e o estado não fica misturado.
  async function fetchAll(projectId: number, seq: number): Promise<boolean> {
    try {
      const [nextSnapshot, nextMovements] = await Promise.all([
        orchestrationApi.fetchOrchestration(projectId),
        orchestrationApi.fetchMovements(projectId),
      ])
      if (seq !== loadSeq) return false
      snapshot.value = nextSnapshot
      movements.value = nextMovements
      loadedProjectId.value = projectId
      loadError.value = null
      return true
    } catch (error) {
      if (seq === loadSeq) {
        loadError.value = errorMessage(error)
        reportError(error)
      }
      return false
    } finally {
      if (seq === loadSeq) loading.value = false
    }
  }

  function load(projectId: number): Promise<boolean> {
    const seq = (loadSeq += 1)
    requestedProjectId = projectId
    resetLoaded()
    loadError.value = null
    loading.value = true
    return fetchAll(projectId, seq)
  }

  function clear(): void {
    loadSeq += 1
    requestedProjectId = null
    resetLoaded()
    loading.value = false
    loadError.value = null
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

  function saveLayer(draft: OrchLayerDraft): Promise<boolean> {
    return write((projectId) => orchestrationApi.saveLayer(projectId, draft))
  }

  function deleteLayer(id: string): Promise<boolean> {
    return write((projectId) => orchestrationApi.deleteLayer(projectId, id))
  }

  function reorderLayers(ids: string[]): Promise<boolean> {
    return write((projectId) => orchestrationApi.reorderLayers(projectId, ids))
  }

  function saveTeam(draft: OrchTeamDraft): Promise<boolean> {
    return write((projectId) => orchestrationApi.saveTeam(projectId, draft))
  }

  function deleteTeam(id: string): Promise<boolean> {
    return write((projectId) => orchestrationApi.deleteTeam(projectId, id))
  }

  function placeAgent(draft: OrchPlacementDraft): Promise<boolean> {
    return write((projectId) => orchestrationApi.placeAgent(projectId, draft))
  }

  function saveRule(draft: OrchRuleDraft): Promise<boolean> {
    return write((projectId) => orchestrationApi.saveRule(projectId, draft))
  }

  function deleteRule(id: number): Promise<boolean> {
    return write((projectId) => orchestrationApi.deleteRule(projectId, id))
  }

  function saveSettings(settings: OrchestrationSettings): Promise<boolean> {
    return write((projectId) => orchestrationApi.saveOrchestrationSettings(projectId, settings))
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
    movements,
    loadedProjectId,
    loading,
    loadError,
    clear,
    load,
    reload,
    saveLayer,
    deleteLayer,
    reorderLayers,
    saveTeam,
    deleteTeam,
    placeAgent,
    saveRule,
    deleteRule,
    saveSettings,
  }
})
