import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import type { Agent } from '../team/events'
import { sendRequest } from '../team/teamApi'
import { useProjects } from '../projects/useProjects'
import { useTeam } from '../team/useTeam'
import type { BacklogItem, BacklogState } from './backlogTypes'
import { useBacklog } from './useBacklog'

export type PlanTab = 'vincular' | 'pedir'
export type PlanDepth = 'rapido' | 'completo'

export const DEFAULT_PLAN_TAB: PlanTab = 'pedir'
export const DEFAULT_DEPTH: PlanDepth = 'completo'
export const PLAN_STEPS = [
  'Ler o item e o backlog',
  'Ler memória e o grafo do código',
  'Quebrar em lotes e tarefas',
  'Estimar e escolher quem faz',
] as const
export const NO_PLAN_TEXT = 'Este item ainda não tem plano. Os agentes só começam depois que ele tiver um.'

const ACCEPTED_PLAN_META = 'proposta aceita'

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
}

const EMPTY_PLAN: ItemPlan = { state: 'vazio' }

export function planPrompt(item: BacklogItem, depth: PlanDepth): string {
  return `Escreva um plano para o item "${item.title}". ${depth === 'rapido' ? 'Um lote.' : 'Com pesquisa.'} Responda em português, em lotes.`
}

export const useItemPlan = defineStore('itemPlan', () => {
  const states = ref<Record<string, ItemPlan>>({})
  const plans = ref<LinkedPlan[]>([])
  const team = useTeam()
  const backlog = useBacklog()
  const projects = useProjects()

  function planOf(itemId: string): ItemPlan {
    return states.value[itemId] ?? EMPTY_PLAN
  }

  function stateOf(itemId: string): BacklogState {
    return planOf(itemId).state
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

  async function request(itemId: string, agent: Agent, depth: PlanDepth): Promise<void> {
    const item = backlog.items.find((current) => current.id === itemId)
    if (!item || isAgentAwaiting(agent)) return
    const entry: ItemPlan = { state: 'gerando', agent }
    states.value[itemId] = entry
    const tracked = states.value[itemId]
    try {
      if (projects.selected === null) {
        throw new Error('Nenhum projeto está selecionado.')
      }
      await sendRequest(planPrompt(item, depth), agent, projects.selected.id)
    } catch (error) {
      if (states.value[itemId] === tracked) {
        states.value[itemId] = tracked.cancelled ? { state: 'vazio' } : { state: 'vazio', error: String(error) }
      }
    }
    // O backend entrega a resposta dentro de send_request: um cancelado ainda sem resposta não a receberá mais.
    if (states.value[itemId] === tracked && tracked.cancelled) reset(itemId)
  }

  function cancel(itemId: string): void {
    const current = states.value[itemId]
    if (current?.state !== 'gerando') return
    // Muda no lugar: request() compara a identidade do registro para limpar o cancelado quando o envio termina.
    current.state = 'vazio'
    current.cancelled = true
  }

  function discard(itemId: string): void {
    reset(itemId)
  }

  function accept(itemId: string): void {
    const item = backlog.items.find((current) => current.id === itemId)
    if (!item || states.value[itemId]?.state !== 'proposto') return
    const plan: LinkedPlan = { id: itemId, name: item.title, meta: ACCEPTED_PLAN_META }
    plans.value = [...plans.value.filter((current) => current.id !== plan.id), plan]
    states.value[itemId] = { state: 'vinculado', plan }
  }

  function link(itemId: string, planId: string): void {
    const plan = plans.value.find((current) => current.id === planId)
    if (plan) states.value[itemId] = { state: 'vinculado', plan }
  }

  function unlink(itemId: string): void {
    reset(itemId)
  }

  function receive(entry: (typeof team.feed)[number]): void {
    if (entry.type === 'notice' || entry.type === 'permission-result') return
    if (entry.type === 'error') {
      const id = oldestAwaiting()
      if (id !== undefined) states.value[id] = states.value[id]?.cancelled ? { state: 'vazio' } : { state: 'vazio', error: entry.message }
      return
    }
    if (entry.to !== 'user' || entry.from === 'user') return
    const id = oldestAwaiting(entry.from)
    if (id === undefined) return
    states.value[id] = states.value[id]?.cancelled
      ? { state: 'vazio' }
      : { state: 'proposto', agent: entry.from, proposal: entry.text }
  }

  watch(
    () => team.feed.length,
    (length, previous) => {
      if (length > previous) team.feed.slice(previous).forEach(receive)
    },
  )

  return { states, plans, planOf, stateOf, isAgentAwaiting, request, cancel, accept, discard, link, unlink }
})
