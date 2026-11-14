import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import {
  type Agent,
  type AgentState,
  type VokiaAgentState,
  type VokiaEvent,
  type VokiaPermission,
  type VokiaRequestSettled,
} from './events'
import { agentName, selectActiveAgents } from './agents'
import { buildTimeline, type FeedEntry, type TimelineItem } from './timeline'
import { answerPermission, type AgentProfile } from './teamApi'
import { useDecisions } from '../decisions/useDecisions'
import { useAgentControl } from '../panel/useAgentControl'
import { clearConversation, fetchDbProblem, loadHistory } from '../projects/projectsApi'
import { useProjects } from '../projects/useProjects'
import { pauseNotice } from '../panel/nowCopy'
import { useRoster } from './useRoster'
import { projectAgentKey } from './projectAgentKey'

export const useFeed = defineStore('feed', () => {
  const feed = ref<FeedEntry[]>([])
  const roster = useRoster()
  const states = ref<Record<Agent, AgentState>>({})
  const permissions = ref<VokiaPermission[]>([])
  const control = useAgentControl()
  const projects = useProjects()
  const pausesLoaded = ref(false)
  const activity = ref<Record<string, AgentState>>({})
  const lastProject = ref<Record<Agent, number | null>>({})
  let requestSettledHandler: ((event: VokiaRequestSettled) => void) | null = null

  function stateOf(agent: Agent): AgentState {
    return states.value[agent] ?? 'idle'
  }

  const thinkingAgent = computed<AgentProfile | undefined>(() =>
    roster.agents.find((agent) => stateOf(agent.id) === 'thinking'),
  )
  const activeAgents = computed<AgentProfile[]>(() => selectActiveAgents(roster.agents, states.value))
  const timeline = computed<TimelineItem[]>(() => buildTimeline(feed.value))

  async function restoreHistory(): Promise<void> {
    try {
      const problem = await fetchDbProblem()
      if (problem !== null) {
        feed.value.push({ type: 'error', message: problem.reason })
        return
      }
      await projects.load()
      await restorePauses()
      if (projects.selected !== null) {
        feed.value = await loadHistory(projects.selected.id)
      }
    } catch (error) {
      feed.value.push({
        type: 'error',
        message: `Não foi possível carregar o histórico: ${String(error)}`,
      })
    }
  }

  async function clearFeed(): Promise<void> {
    const project = projects.selected
    if (project === null) {
      return
    }
    await clearConversation(project.id)
    feed.value = []
  }

  async function restorePauses(): Promise<void> {
    try {
      await control.load()
      pausesLoaded.value = true
    } catch (error) {
      feed.value.push({
        type: 'error',
        message: `Não foi possível carregar as pausas dos agentes: ${String(error)}`,
      })
    }
  }

  function onRequestSettled(handler: (event: VokiaRequestSettled) => void): void {
    requestSettledHandler = handler
  }

  function apply(event: VokiaEvent): void {
    switch (event.type) {
      case 'message':
      case 'error':
        feed.value.push(event)
        break
      case 'agent-state':
        states.value[event.agent] = event.state
        recordActivity(event)
        break
      case 'permission':
        permissions.value.push(event)
        break
      case 'permission-closed': {
        const closed = permissions.value.find((permission) => permission.id === event.id)
        if (closed) {
          feed.value.push({
            type: 'permission-result',
            agent: closed.agent,
            detail: closed.detail,
            outcome: event.outcome,
            time: Date.now(),
          })
        }
        permissions.value = permissions.value.filter((permission) => permission.id !== event.id)
        break
      }
      case 'decision':
        useDecisions().apply(event)
        break
      case 'request-settled':
        requestSettledHandler?.(event)
        break
      case 'plan-usage':
        break
    }
  }

  function recordActivity({ agent, state, projectId }: VokiaAgentState): void {
    if (projectId === null) {
      return
    }
    lastProject.value[agent] = projectId
    activity.value[projectAgentKey(projectId, agent)] = state
  }

  function projectsOf(agent: Agent): number[] {
    return Object.entries(activity.value)
      .filter(([key, state]) => state !== 'idle' && key.endsWith(`:${agent}`))
      .map(([key]) => Number(key.split(':')[0]))
  }

  async function togglePause(agent: Agent): Promise<void> {
    try {
      const pausing = !control.isPaused(agent)
      await (pausing ? control.pause(agent) : control.resume(agent))
      feed.value.push({ type: 'notice', text: pauseNotice(agentName(agent), pausing), time: Date.now() })
    } catch (error) {
      feed.value.push({ type: 'error', message: String(error) })
    }
  }

  async function decidePermission(id: string, allow: boolean): Promise<void> {
    try {
      await answerPermission(id, allow)
    } catch (error) {
      feed.value.push({ type: 'error', message: String(error) })
      permissions.value = permissions.value.filter((permission) => permission.id !== id)
    }
  }

  function notify(text: string): void {
    feed.value.push({ type: 'notice', text, time: Date.now() })
  }

  return {
    feed,
    states,
    stateOf,
    thinkingAgent,
    activeAgents,
    timeline,
    permissions,
    activity,
    lastProject,
    pausesLoaded,
    projectsOf,
    apply,
    restoreHistory,
    restorePauses,
    clearFeed,
    togglePause,
    decidePermission,
    notify,
    onRequestSettled,
  }
})
