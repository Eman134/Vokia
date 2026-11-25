import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { Agent, TurnPlan, VokiaMessage, VokiaTurnPlanUpdated } from './events'
import { projectAgentKey } from './projectAgentKey'
import { useProjects } from '../projects/useProjects'
import { useFeed } from './useFeed'

export type TurnPlanCardStatus = 'active' | 'completed' | 'partial' | 'replaced'

export interface TurnPlanEntry {
  projectId: number
  agent: Agent
  cardId?: string
  turnId: string
  plan: TurnPlan
  active: boolean
  updatedAt: number
}

export function statusOfTurnPlan(
  plan: TurnPlan,
  options: { active: boolean; replaced?: boolean },
): TurnPlanCardStatus {
  if (options.replaced) return 'replaced'
  if (options.active) return 'active'
  const allCompleted = plan.steps.length > 0 && plan.steps.every((step) => step.status === 'completed')
  return allCompleted ? 'completed' : 'partial'
}

export const useTurnPlans = defineStore('turn-plans', () => {
  const byKey = ref<Record<string, TurnPlanEntry>>({})
  const projects = useProjects()

  function apply(event: VokiaTurnPlanUpdated): void {
    if (event.plan.steps.length === 0) return
    const key = projectAgentKey(event.projectId, event.agent)
    byKey.value[key] = {
      projectId: event.projectId,
      agent: event.agent,
      cardId: event.cardId,
      turnId: event.turnId,
      plan: event.plan,
      active: true,
      updatedAt: Date.now(),
    }
  }

  function markFinished(projectId: number | null, agent: Agent): void {
    if (projectId === null) return
    const key = projectAgentKey(projectId, agent)
    const entry = byKey.value[key]
    if (entry) {
      byKey.value[key] = { ...entry, active: false }
    }
  }

  function forgetProject(projectId: number): void {
    const prefix = `${projectId}:`
    const next: Record<string, TurnPlanEntry> = {}
    for (const [key, entry] of Object.entries(byKey.value)) {
      if (!key.startsWith(prefix)) {
        next[key] = entry
      }
    }
    byKey.value = next
  }

  function livePlanOf(agent: Agent, itemId?: string): TurnPlanEntry | undefined {
    const project = projects.selected
    if (project === null) return undefined
    const entry = byKey.value[projectAgentKey(project.id, agent)]
    if (!entry || !entry.active || entry.plan.steps.length === 0) return undefined
    if (itemId !== undefined && entry.cardId !== itemId) return undefined
    return entry
  }

  function planOf(agent: Agent, projectId?: number): TurnPlanEntry | undefined {
    const feed = useFeed()
    const resolvedProjectId = projectId ?? projects.selected?.id ?? feed.lastProject[agent] ?? null
    if (resolvedProjectId !== null) {
      const entry = byKey.value[projectAgentKey(resolvedProjectId, agent)]
      if (entry && entry.plan.steps.length > 0) return entry
    }
    // Fallback para o último plano do agente já presente no histórico do projeto aberto.
    const activeProjectId = resolvedProjectId ?? projects.selected?.id ?? null
    if (activeProjectId === null) return undefined
    for (let i = feed.feed.length - 1; i >= 0; i -= 1) {
      const candidate = feed.feed[i]
      if (
        candidate?.type === 'message' &&
        candidate.from === agent &&
        candidate.projectId === activeProjectId &&
        candidate.activity?.plan &&
        candidate.activity.plan.steps.length > 0
      ) {
        return {
          projectId: activeProjectId,
          agent,
          cardId: candidate.itemId,
          turnId: `history-${candidate.time}`,
          plan: candidate.activity.plan,
          active: false,
          updatedAt: candidate.time,
        }
      }
    }
    return undefined
  }

  // Um plano de mensagem antiga fica marcado como substituído assim que um turno mais novo
  // do mesmo agente (ao vivo ou em mensagem posterior no feed) publica um novo plano.
  function isMessagePlanReplaced(message: VokiaMessage): boolean {
    if (message.from === 'user' || !message.activity?.plan || message.activity.plan.steps.length === 0) {
      return false
    }
    const projectId = message.projectId ?? projects.selected?.id ?? null
    if (projectId !== null) {
      const live = byKey.value[projectAgentKey(projectId, message.from)]
      if (live && live.active && live.plan.steps.length > 0) {
        if (message.itemId === undefined || live.cardId === undefined || live.cardId === message.itemId) {
          return true
        }
      }
    }
    const feed = useFeed()
    const index = feed.feed.lastIndexOf(message)
    if (index === -1) return false
    for (let i = index + 1; i < feed.feed.length; i += 1) {
      const later = feed.feed[i]
      if (
        later?.type === 'message' &&
        later.from === message.from &&
        later.projectId === message.projectId &&
        later.itemId === message.itemId &&
        later.activity?.plan &&
        later.activity.plan.steps.length > 0
      ) {
        return true
      }
    }
    return false
  }

  return {
    byKey,
    apply,
    markFinished,
    forgetProject,
    livePlanOf,
    planOf,
    isMessagePlanReplaced,
  }
})
