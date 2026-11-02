import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import type { Agent } from '../team/events'
import { agentName, DEFAULT_RECIPIENT, useTeam, type RunResult } from '../team/useTeam'
import { useProjects } from '../projects/useProjects'
import { DEFAULT_COLUMN, type BacklogColumn, type BacklogItem } from './backlogTypes'
import { useBacklog, type RunColumn } from './useBacklog'
import { useItemPlan } from './useItemPlan'

const RUN_TITLE_PREFIX = 'Execute o item'
const DESCRIPTION_HEADING = 'Descrição'
const CRITERIA_HEADING = 'Critérios'
const PLAN_HEADING = 'Plano'
const RUN_CLOSING = 'Avise a Iara ao começar e ao terminar.'
const SENT_NOTE = (agent: string) => `Execução enviada para ${agent}.`
const QUEUED_NOTE = (agent: string) => `${agent} está ocupado; o pedido espera na fila dele.`
const DONE_NOTE = (agent: string) => `Execução concluída por ${agent}.`
const NOT_SENT_NOTE = 'A execução não foi enviada.'
const FAILED_NOTE = 'A execução falhou; veja o erro no feed.'
const CANCELLED_NOTE = 'Execução cancelada na fila.'
const INTERRUPTED_NOTE = 'Execução interrompida: o app foi fechado ou o projeto trocado antes do fim.'
const MOVED_NOTE = (label: string) => `Execução terminou (${label}) com o card já movido.`
const RESULT_LABEL: Record<RunResult, string> = { finished: 'concluída', failed: 'falhou', cancelled: 'cancelada' }
// Com o card nessas colunas há execução (ou resto dela): Rodar não envia outro pedido.
const BUSY_COLUMNS: readonly BacklogColumn[] = ['In progress', 'In review']

interface Settlement {
  result: RunResult
  agent: Agent
}

interface AwayRun {
  projectId: number
  itemId: string
  settlement: Settlement
}

export function buildRunPrompt(item: BacklogItem, planText: string): string {
  const sections = [`${RUN_TITLE_PREFIX} "${item.title}".`]
  if (item.description) sections.push(`${DESCRIPTION_HEADING}: ${item.description}`)
  const criteria = item.criteria ?? []
  if (criteria.length > 0) {
    sections.push(`${CRITERIA_HEADING}:\n${criteria.map((criterion) => `- ${criterion.text}`).join('\n')}`)
  }
  if (planText) sections.push(`${PLAN_HEADING}:\n${planText}`)
  sections.push(RUN_CLOSING)
  return sections.join('\n\n')
}

// Ids de item se repetem entre projetos: a chave de execução inclui o projeto.
function runKey(projectId: number, itemId: string): string {
  return `${projectId}:${itemId}`
}

function outcomeOf(result: RunResult, agent: Agent): { column: RunColumn; note: string } {
  switch (result) {
    case 'finished':
      return { column: 'In review', note: DONE_NOTE(agentName(agent)) }
    case 'failed':
      return { column: 'Ready', note: FAILED_NOTE }
    case 'cancelled':
      return { column: 'Ready', note: CANCELLED_NOTE }
  }
}

export const useRunItem = defineStore('runItem', () => {
  const running = ref<Record<string, true>>({})
  // Término de execução de projeto que não está carregado: aplicado quando ele voltar a carregar.
  const awayRuns = new Map<string, AwayRun>()
  const backlog = useBacklog()
  const itemPlan = useItemPlan()
  const team = useTeam()
  const projects = useProjects()

  function findItem(itemId: string): BacklogItem | undefined {
    return backlog.items.find((current) => current.id === itemId)
  }

  function isLoaded(projectId: number): boolean {
    return backlog.loadedProjectId === projectId
  }

  function isRunning(itemId: string): boolean {
    const projectId = backlog.loadedProjectId
    return projectId !== null && running.value[runKey(projectId, itemId)] === true
  }

  function canRun(itemId: string): boolean {
    const item = findItem(itemId)
    const projectId = backlog.loadedProjectId
    return (
      item !== undefined &&
      !item.error &&
      projectId !== null &&
      projectId === projects.selected?.id &&
      itemPlan.stateOf(itemId) === 'vinculado' &&
      !BUSY_COLUMNS.includes(item.column ?? DEFAULT_COLUMN) &&
      !isRunning(itemId)
    )
  }

  // Sem o texto do plano próprio (plano vinculado de outro item), vale o do plano de origem.
  function planTextOf(itemId: string): string {
    const current = itemPlan.planOf(itemId)
    if (current.proposal) return current.proposal
    const origin = current.plan ? itemPlan.planOf(current.plan.id).proposal : undefined
    return origin || (current.plan?.name ?? '')
  }

  function applySettlement(itemId: string, { result, agent }: Settlement): void {
    const item = findItem(itemId)
    if (item === undefined) return
    if (item.column !== 'In progress') {
      backlog.addActivity(itemId, MOVED_NOTE(RESULT_LABEL[result]))
      return
    }
    const { column, note } = outcomeOf(result, agent)
    backlog.setRunColumn(itemId, column, note)
  }

  function settle(projectId: number, itemId: string, settlement: Settlement): void {
    delete running.value[runKey(projectId, itemId)]
    if (isLoaded(projectId)) {
      applySettlement(itemId, settlement)
      return
    }
    awayRuns.set(runKey(projectId, itemId), { projectId, itemId, settlement })
  }

  function noteQueued(projectId: number, itemId: string, agent: Agent): void {
    if (isLoaded(projectId)) backlog.addActivity(itemId, QUEUED_NOTE(agentName(agent)))
  }

  // Execução que ficou In progress sem resultado em memória (app fechado ou projeto trocado) volta a Ready.
  function reconcile(projectId: number): void {
    for (const [key, away] of awayRuns) {
      if (away.projectId !== projectId) continue
      awayRuns.delete(key)
      applySettlement(away.itemId, away.settlement)
    }
    for (const item of backlog.items) {
      if (item.column === 'In progress' && running.value[runKey(projectId, item.id)] !== true) {
        backlog.setRunColumn(item.id, 'Ready', INTERRUPTED_NOTE)
      }
    }
  }

  watch(
    () => backlog.loadedProjectId,
    (projectId) => {
      if (projectId !== null) reconcile(projectId)
    },
  )

  async function run(itemId: string): Promise<void> {
    const item = findItem(itemId)
    const projectId = backlog.loadedProjectId
    if (item === undefined || projectId === null || !canRun(itemId)) return
    const agent = itemPlan.planOf(itemId).agent ?? DEFAULT_RECIPIENT
    running.value[runKey(projectId, itemId)] = true
    backlog.setRunColumn(itemId, 'In progress', SENT_NOTE(agentName(agent)))
    const accepted = await team.send(buildRunPrompt(item, planTextOf(itemId)), agent, {
      projectId,
      onQueued: () => noteQueued(projectId, itemId, agent),
      onSettled: (result) => settle(projectId, itemId, { result, agent }),
    })
    if (!accepted) {
      delete running.value[runKey(projectId, itemId)]
      if (isLoaded(projectId)) backlog.setRunColumn(itemId, 'Ready', NOT_SENT_NOTE)
    }
  }

  return { canRun, isRunning, run }
})
