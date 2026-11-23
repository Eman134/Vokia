import { ref } from 'vue'
import { defineStore } from 'pinia'
import { useFeed } from '../team/useFeed'
import type { Agent, VokiaBatchProgress } from '../team/events'
import { syncFlowDrafts } from './flowSync'
import * as orchestrationApi from './orchestrationApi'
import type { FlowProblem, OrchPlacementDraft, OrchTaskStage, OrchTeamDraft, OrchestrationSnapshot, TeamFlowDraft } from './orchestrationTypes'

// A orquestração é global: um só snapshot para todos os projetos, recarregado a cada evento de mudança.
function parseProblems(error: unknown): FlowProblem[] {
  try {
    const parsed: unknown = JSON.parse(errorText(error))
    return Array.isArray(parsed) ? (parsed as FlowProblem[]) : []
  } catch {
    return []
  }
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export const useOrchestration = defineStore('orchestration', () => {
  const snapshot = ref<OrchestrationSnapshot | null>(null)
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

  // O evento traz a etapa já gravada; o snapshot só é recarregado nas mudanças de configuração.
  function applyTaskStage(stage: OrchTaskStage): void {
    const current = snapshot.value
    if (current === null) return
    const others = current.taskStages.filter((known) => known.projectId !== stage.projectId || known.itemId !== stage.itemId)
    current.taskStages = [...others, stage]
  }
  const feed = useFeed()
  let loadSeq = 0

  function reportError(error: unknown): void {
    feed.feed.push({ type: 'error', message: errorText(error) })
  }

  // Resposta de carga anterior é descartada pela sequência; a tela não é limpa entre cargas.
  async function load(): Promise<boolean> {
    const seq = (loadSeq += 1)
    try {
      const next = await orchestrationApi.fetchOrchestration()
      if (seq !== loadSeq) return false
      const previousTeamIds = snapshot.value?.teams.map((team) => team.id) ?? []
      snapshot.value = next
      syncFlowDrafts(next, previousTeamIds)
      return true
    } catch (error) {
      if (seq === loadSeq) reportError(error)
      return false
    }
  }

  // Escrita não mexe no estado: a tela muda quando o evento orchestration-changed recarregar.
  async function write(operation: () => Promise<unknown>): Promise<boolean> {
    try {
      await operation()
      return true
    } catch (error) {
      reportError(error)
      return false
    }
  }

  function placeAgent(draft: OrchPlacementDraft): Promise<boolean> {
    return write(() => orchestrationApi.placeAgent(draft))
  }

  function saveTeam(draft: OrchTeamDraft, leaderAgent: Agent | null): Promise<boolean> {
    return write(() => orchestrationApi.saveTeam(draft, leaderAgent))
  }

  function deleteTeam(id: string): Promise<boolean> {
    return write(() => orchestrationApi.deleteTeam(id))
  }

  // Salva a estrutura do fluxo. Recusa do backend volta como a lista de problemas (JSON); outro erro vai ao feed.
  async function saveFlow(teamId: string, flow: TeamFlowDraft): Promise<{ saved: boolean; problems: FlowProblem[] }> {
    try {
      snapshot.value = await orchestrationApi.saveTeamFlow(teamId, flow)
      return { saved: true, problems: [] }
    } catch (error) {
      const problems = parseProblems(error)
      if (problems.length === 0) reportError(error)
      return { saved: false, problems }
    }
  }

  void load()

  return {
    snapshot,
    batches,
    applyBatchProgress,
    applyTaskStage,
    load,
    reload: load,
    saveFlow,
    placeAgent,
    saveTeam,
    deleteTeam,
  }
})
