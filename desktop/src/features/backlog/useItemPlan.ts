import { nextTick, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import type { Agent } from '../team/events'
import { sendRequest, type RequestOutcome } from '../team/teamApi'
import { useProjects } from '../projects/useProjects'
import { useDecisions } from '../decisions/useDecisions'
import { agentName } from '../team/agents'
import { useFeed } from '../team/useFeed'
import { importPlan, updateItemPlan, type PlanArtifactDetail } from '../artifacts/artifactApi'
import { createBacklogItem, EMPTY_PLAN_STATE, type ItemPlanEntry, type ItemPlanInput } from './backlogApi'
import {
  DECISION_QUESTION_PREFIX,
  DEFAULT_COLUMN,
  ORIGIN_USER,
  READY_COLUMN,
  type TaskCardAction,
  type BacklogItem,
  type BacklogState,
} from './backlogTypes'
import { useBacklog } from './useBacklog'
import { useTriage } from './useTriage'
import {
  buildAdjustPrompt,
  buildImportPrompt,
  buildPlanPrompt,
  isFinalReply,
  parseProposal,
  type ParsedProposal,
} from './planProposal'

export type PlanTab = 'vincular' | 'pedir'
export type PlanDepth = 'rapido' | 'completo'

export const DEFAULT_PLAN_TAB: PlanTab = 'pedir'
export const PLAN_TAB_BY_ACTION: Record<Exclude<TaskCardAction, 'iniciar'>, PlanTab> = {
  triar: 'pedir',
  revisar: 'pedir',
  vincular: 'vincular',
}
export const DEFAULT_DEPTH: PlanDepth = 'completo'
export const PLAN_STEPS = [
  'Ler o item e o backlog',
  'Ler memória e o grafo do código',
  'Quebrar em lotes e tarefas',
  'Estimar e escolher quem faz',
] as const
export const NO_PLAN_TEXT = 'Este item ainda não tem plano. O plano é opcional: você pode iniciar a task direto.'

export const REVIEWED_PLAN_META = 'proposta aceita'
const PLAN_LINE_SEPARATOR = ' · '
const PLAN_LINE_PROPOSED = 'proposta'
export const PLAN_LINE_GENERATING = 'escrevendo o plano…'
const NO_REPLY_ERROR = 'O agente terminou sem mandar um plano.'
const NO_PROJECT_ERROR = 'Nenhum projeto está selecionado.'

export interface LinkedPlan {
  id: string
  name: string
  meta: string
}

export interface ItemPlan {
  state: BacklogState
  proposal?: string
  plan?: LinkedPlan
  agent?: Agent
  error?: string
  cancelled?: boolean
  interim?: string
}

const EMPTY_PLAN: ItemPlan = { state: 'vazio' }

function toItemPlan(entry: ItemPlanEntry): ItemPlan {
  return { state: entry.state, proposal: entry.proposal, plan: entry.plan, agent: entry.agent }
}

function linkedPlansOf(entries: ItemPlanEntry[]): LinkedPlan[] {
  const byId = new Map<string, LinkedPlan>()
  entries.forEach((entry) => {
    if (entry.plan) byId.set(entry.plan.id, entry.plan)
  })
  return [...byId.values()]
}

export const useItemPlan = defineStore('itemPlan', () => {
  const states = ref<Record<string, ItemPlan>>({})
  const plans = ref<LinkedPlan[]>([])
  const dialogItemId = ref<string | null>(null)
  const dialogTab = ref<PlanTab>(DEFAULT_PLAN_TAB)
  const feed = useFeed()
  const backlog = useBacklog()
  const triage = useTriage()
  const projects = useProjects()
  const decisions = useDecisions()

  function planOf(itemId: string): ItemPlan {
    return states.value[itemId] ?? EMPTY_PLAN
  }

  function stateOf(itemId: string): BacklogState {
    return planOf(itemId).state
  }

  function stateFor(item: BacklogItem): BacklogState {
    return states.value[item.id]?.state ?? item.state
  }

  function lineFor(item: BacklogItem): string | undefined {
    const state = stateFor(item)
    const saved = states.value[item.id]
    if (state === 'vazio') return undefined
    if (state === 'vinculado' && saved?.plan) return [saved.plan.name, saved.plan.meta].join(PLAN_LINE_SEPARATOR)
    if (state === 'proposto' && saved?.agent) return [agentName(saved.agent), PLAN_LINE_PROPOSED].join(PLAN_LINE_SEPARATOR)
    if (state === 'gerando' && saved?.agent) {
      const open = item.questions?.filter((question) => question.id.startsWith(DECISION_QUESTION_PREFIX) && question.selected === undefined).length ?? 0
      const text = open > 0 ? `${open} ${open === 1 ? 'pergunta' : 'perguntas'} para você` : PLAN_LINE_GENERATING
      return [agentName(saved.agent), text].join(PLAN_LINE_SEPARATOR)
    }
    return item.plan
  }

  // Pedir e Vincular são modais distintos: o modo vale enquanto o diálogo estiver aberto.
  function openDialog(itemId: string, tab: PlanTab): void {
    dialogTab.value = tab
    dialogItemId.value = itemId
  }

  function closeDialog(): void {
    dialogItemId.value = null
    dialogTab.value = DEFAULT_PLAN_TAB
  }

  function persist(itemId: string): void {
    const current = states.value[itemId]
    const input: ItemPlanInput =
      current?.state === 'proposto' || current?.state === 'vinculado'
        ? { state: current.state, proposal: current.proposal, plan: current.plan, agent: current.agent }
        : { state: EMPTY_PLAN_STATE }
    backlog.savePlan(itemId, input)
  }

  function clearPlan(itemId: string): void {
    reset(itemId)
    persist(itemId)
  }

  function reset(itemId: string): void {
    delete states.value[itemId]
  }

  // Item cancelado continua esperando a resposta do agente, que é descartada ao chegar.
  function isAwaiting(current: ItemPlan | undefined, agent?: Agent): boolean {
    return (
      current?.agent !== undefined &&
      (agent === undefined || current.agent === agent) &&
      (current.state === 'gerando' || current.cancelled === true)
    )
  }

  function isAgentAwaiting(agent: Agent): boolean {
    return Object.values(states.value).some((current) => isAwaiting(current, agent))
  }

  // Em ordem de pedido: com dois itens esperando o mesmo agente, a resposta é do mais antigo.
  function oldestAwaiting(agent?: Agent): string | undefined {
    return Object.keys(states.value).find((id) => isAwaiting(states.value[id], agent))
  }

  function hasPendingQuestion(itemId: string): boolean {
    return decisions.pending.some((decision) => decision.itemId === itemId && decision.projectId === projects.selected?.id)
  }

  function promoteInterim(itemId: string): void {
    const current = states.value[itemId]
    if (current?.state !== 'gerando' || current.agent === undefined) return
    if (current.interim === undefined) {
      states.value[itemId] = { state: 'vazio', error: NO_REPLY_ERROR }
      return
    }
    states.value[itemId] = { state: 'proposto', agent: current.agent, proposal: current.interim }
    persist(itemId)
  }

  function selectedProjectId(): number {
    const projectId = projects.selected?.id
    if (projectId === undefined) throw new Error(NO_PROJECT_ERROR)
    return projectId
  }

  async function dispatch(item: BacklogItem, agent: Agent, prompt: string): Promise<void> {
    const itemId = item.id
    states.value[itemId] = { state: 'gerando', agent }
    const tracked = states.value[itemId]
    let outcome: RequestOutcome | undefined
    try {
      outcome = await sendRequest(prompt, agent, selectedProjectId(), undefined, itemId)
    } catch (error) {
      if (states.value[itemId] === tracked) {
        states.value[itemId] = tracked.cancelled ? { state: 'vazio' } : { state: 'vazio', error: String(error) }
      }
    }
    // Deixa o watcher do feed processar a resposta antes de decidir pelo atalho.
    await nextTick()
    if (states.value[itemId] !== tracked) return
    // O backend entrega a resposta dentro de send_request: um cancelado ainda sem resposta não a receberá mais.
    if (tracked.cancelled) reset(itemId)
    else if (outcome === 'finished' && !hasPendingQuestion(itemId)) promoteInterim(itemId)
  }

  async function request(itemId: string, agent: Agent, depth: PlanDepth, note = ''): Promise<void> {
    const item = backlog.items.find((current) => current.id === itemId)
    if (!item || isAgentAwaiting(agent)) return
    backlog.setPlanNote(itemId, note)
    void triage.requestTriage(item)
    await dispatch(item, agent, buildPlanPrompt(item, depth))
  }

  async function adjust(itemId: string, instruction: string): Promise<void> {
    const item = backlog.items.find((current) => current.id === itemId)
    const current = states.value[itemId]
    const text = instruction.trim()
    if (!item || current?.state !== 'proposto' || !current.agent || !current.proposal || text === '') return
    if (isAgentAwaiting(current.agent)) return
    await dispatch(item, current.agent, buildAdjustPrompt(item, current.proposal, text))
  }

  // O card nasce antes da resposta para o fluxo de geração acompanhar o estado; retryItemId reaproveita o card de uma triagem que falhou.
  async function importFree(title: string, text: string, agent: Agent, retryItemId?: string): Promise<string | undefined> {
    if (isAgentAwaiting(agent)) return undefined
    const projectId = selectedProjectId()
    let item = retryItemId === undefined ? undefined : backlog.items.find((current) => current.id === retryItemId)
    if (!item) {
      const created = await createBacklogItem(projectId, { title, origin: ORIGIN_USER, links: [] })
      await backlog.reloadAfterAgentChange(projectId)
      item = backlog.items.find((current) => current.id === created.id) ?? created
    }
    void dispatch(item, agent, buildImportPrompt(title, text))
    return item.id
  }

  async function importExact(title: string, protocol: string): Promise<BacklogItem> {
    const projectId = selectedProjectId()
    const item = await importPlan(projectId, title, protocol)
    await backlog.reloadAfterAgentChange(projectId)
    return item
  }

  async function edit(itemId: string, proposal: string, expectedUpdatedAt?: number): Promise<PlanArtifactDetail> {
    const artifact = await updateItemPlan(selectedProjectId(), itemId, proposal, expectedUpdatedAt)
    const current = states.value[itemId]
    if (current) current.proposal = proposal
    return artifact
  }

  function cancel(itemId: string): void {
    const current = states.value[itemId]
    if (current?.state !== 'gerando') return
    // Muda no lugar: request() compara a identidade do registro para limpar o cancelado quando o envio termina.
    current.state = 'vazio'
    current.cancelled = true
  }

  function discard(itemId: string): void {
    clearPlan(itemId)
  }

  function promoteToReady(itemId: string): void {
    if (backlog.items.find((current) => current.id === itemId)?.column === DEFAULT_COLUMN) backlog.moveTo(itemId, READY_COLUMN)
  }

  function accept(itemId: string): void {
    const item = backlog.items.find((current) => current.id === itemId)
    if (!item || states.value[itemId]?.state !== 'proposto') return
    const plan: LinkedPlan = { id: itemId, name: item.title, meta: REVIEWED_PLAN_META }
    plans.value = [...plans.value.filter((current) => current.id !== plan.id), plan]
    states.value[itemId] = {
      state: 'vinculado',
      plan,
      proposal: states.value[itemId]?.proposal,
      agent: states.value[itemId]?.agent,
    }
    persist(itemId)
    backlog.addTags(itemId, parseProposal(states.value[itemId]?.proposal ?? '').tags)
    promoteToReady(itemId)
  }

  function link(itemId: string, planId: string): void {
    const plan = plans.value.find((current) => current.id === planId)
    if (!plan) return
    states.value[itemId] = { state: 'vinculado', plan, agent: states.value[planId]?.agent }
    persist(itemId)
    const item = backlog.items.find((current) => current.id === itemId)
    if (item) void triage.requestTriage(item)
    if (plan.meta === REVIEWED_PLAN_META) promoteToReady(itemId)
  }

  function unlink(itemId: string): void {
    const column = backlog.items.find((current) => current.id === itemId)?.column
    clearPlan(itemId)
    if (column === READY_COLUMN) backlog.moveTo(itemId, DEFAULT_COLUMN)
  }

  function isReviewed(itemId: string): boolean {
    return states.value[itemId]?.state === 'vinculado' && states.value[itemId]?.plan?.meta === REVIEWED_PLAN_META
  }

  function receive(entry: (typeof feed.feed)[number]): void {
    if (entry.type === 'notice' || entry.type === 'permission-result' || entry.type === 'task-result') return
    if (entry.type === 'error') {
      const id = oldestAwaiting()
      if (id !== undefined) states.value[id] = states.value[id]?.cancelled ? { state: 'vazio' } : { state: 'vazio', error: entry.message }
      return
    }
    if (entry.to !== 'user' || entry.from === 'user') return
    const id = oldestAwaiting(entry.from)
    if (id === undefined) return
    if (states.value[id]?.cancelled) {
      states.value[id] = { state: 'vazio' }
      return
    }
    if (isFinalReply(entry.text)) {
      states.value[id] = { state: 'proposto', agent: entry.from, proposal: entry.text }
      persist(id)
      return
    }
    const current = states.value[id]
    if (current) current.interim = entry.text
  }

  watch(
    () => feed.feed.length,
    (length, previous) => {
      if (length > previous) feed.feed.slice(previous).forEach(receive)
    },
  )

  watch(
    () => backlog.planEntries,
    (entries) => {
      const awaiting = Object.entries(states.value).filter(
        ([itemId, current]) =>
          (current.state === 'gerando' || current.cancelled === true) &&
          backlog.items.some((item) => item.id === itemId),
      )
      states.value = {
        ...Object.fromEntries(entries.map((entry) => [entry.itemId, toItemPlan(entry)])),
        ...Object.fromEntries(awaiting),
      }
      plans.value = linkedPlansOf(entries)
    },
  )

  function proposalFor(item: BacklogItem): ParsedProposal | undefined {
    const proposal = states.value[item.id]?.proposal
    return proposal ? parseProposal(proposal) : undefined
  }

  return {
    states,
    plans,
    dialogItemId,
    dialogTab,
    planOf,
    stateOf,
    stateFor,
    lineFor,
    proposalFor,
    openDialog,
    closeDialog,
    isAgentAwaiting,
    isReviewed,
    request,
    hasPendingQuestion,
    promoteInterim,
    adjust,
    importFree,
    importExact,
    edit,
    cancel,
    accept,
    discard,
    link,
    unlink,
  }
})
