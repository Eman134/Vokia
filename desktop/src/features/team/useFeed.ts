import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import {
  type Agent,
  type AgentState,
  type TurnActivity,
  type VokiaAgentState,
  type VokiaEvent,
  type VokiaGoalState,
  type VokiaPermission,
  type VokiaRequestSettled,
  type VokiaTaskResult,
} from './events'
import { agentName, selectActiveAgents } from './agents'
import { buildTimeline, screenEntries, type ConversationScreen, type FeedEntry, type TimelineItem } from './timeline'
import { useChannels } from '../channels/useChannels'
import { GENERAL_CHANNEL_NAME } from '../channels/channelRouting'
import { answerPermission, type AgentProfile } from './teamApi'
import { useDecisions } from '../decisions/useDecisions'
import { useAgentControl } from '../panel/useAgentControl'
import { clearConversation, fetchDbProblem, loadHistory } from '../projects/projectsApi'
import { useProjects } from '../projects/useProjects'
import { pauseNotice } from '../panel/nowCopy'
import { useRoster } from './useRoster'
import { projectAgentKey } from './projectAgentKey'
import { goalNotice } from './goalCopy'
import { useConsole } from '../console/useConsole'
import { useBacklog } from '../backlog/useBacklog'
import { useOrchestration } from '../orchestration/useOrchestration'
import { useAgentActivity } from '../teammap/useAgentActivity'
import { useUsageLimit } from '../usage/useUsageLimit'

const GENERAL_SCREEN: ConversationScreen = { kind: 'channel', channel: GENERAL_CHANNEL_NAME }

export const useFeed = defineStore('feed', () => {
  const feed = ref<FeedEntry[]>([])
  const roster = useRoster()
  const states = ref<Record<Agent, AgentState>>({})
  const stateSince = ref<Partial<Record<Agent, number>>>({})
  const liveActivity = ref<Record<string, TurnActivity>>({})
  const permissions = ref<VokiaPermission[]>([])
  const control = useAgentControl()
  const projects = useProjects()
  const pausesLoaded = ref(false)
  const activity = ref<Record<string, AgentState>>({})
  const lastProject = ref<Record<Agent, number | null>>({})
  const screen = ref<ConversationScreen>(GENERAL_SCREEN)
  let requestSettledHandler: ((event: VokiaRequestSettled) => void) | null = null

  function stateOf(agent: Agent): AgentState {
    return states.value[agent] ?? 'idle'
  }

  const thinkingAgent = computed<AgentProfile | undefined>(() =>
    roster.agents.find((agent) => stateOf(agent.id) === 'thinking'),
  )
  const pendingAgent = computed<AgentProfile | undefined>(
    () => thinkingAgent.value ?? roster.agents.find((agent) => stateOf(agent.id) === 'sending'),
  )
  const pendingSince = computed<number | undefined>(() =>
    pendingAgent.value ? stateSince.value[pendingAgent.value.id] : undefined,
  )
  const activeAgents = computed<AgentProfile[]>(() => selectActiveAgents(roster.agents, states.value))
  const channels = useChannels()
  const timeline = computed<TimelineItem[]>(() =>
    buildTimeline(screenEntries(feed.value, screen.value, channels.channels)),
  )

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
    useConsole().forgetProject(project.id)
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
        feed.value.push(event)
        if (event.isError && event.from !== 'user') {
          useUsageLimit().suggestFromLimitMessage(event.from)
        }
        break
      case 'error':
        feed.value.push(event)
        break
      case 'agent-activity':
        liveActivity.value[projectAgentKey(event.projectId, event.agent)] = event.activity
        break
      case 'agent-state':
        recordState(event)
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
      case 'permission-by-rule':
        feed.value.push({
          type: 'permission-result',
          agent: event.agent,
          detail: event.detail,
          outcome: 'project-rule',
          time: Date.now(),
        })
        break
      case 'decision':
        useDecisions().apply(event)
        break
      case 'request-settled':
        requestSettledHandler?.(event)
        break
      case 'console-line':
        useConsole().apply(event)
        break
      case 'goal-state':
        showGoalState(event)
        break
      case 'task-result':
        showTaskResult(event)
        break
      case 'backlog-changed':
        void useBacklog().reloadAfterAgentChange(event.projectId)
        void useAgentActivity().load()
        break
      case 'agent-named':
        roster.nameInstance(event.agent, event.name)
        break
      case 'orchestration-changed':
        void useOrchestration().reload()
        break
      case 'batch-progress':
        useOrchestration().applyBatchProgress(event)
        break
      case 'plan-usage':
        break
      case 'usage-limit':
        useUsageLimit().apply(event)
        break
    }
  }

  function showTaskResult(event: VokiaTaskResult): void {
    if (event.projectId === projects.selected?.id) {
      feed.value.push(event)
    }
  }

  // O aviso não leva projectId no feed: só entra quando o projeto do evento é o aberto.
  function showGoalState(event: VokiaGoalState): void {
    if (event.projectId !== projects.selected?.id) {
      return
    }
    const text = goalNotice(event)
    if (text !== null) {
      notify(text)
    }
  }

  function recordState({ agent, state }: VokiaAgentState): void {
    if (stateOf(agent) !== state) {
      stateSince.value[agent] = Date.now()
    }
    states.value[agent] = state
  }

  function recordActivity({ agent, state, projectId }: VokiaAgentState): void {
    if (projectId === null) {
      return
    }
    lastProject.value[agent] = projectId
    activity.value[projectAgentKey(projectId, agent)] = state
    if (state === 'idle') {
      dropLiveActivity(projectId, agent)
    }
  }

  function dropLiveActivity(projectId: number | null, agent: Agent): void {
    if (projectId === null) {
      return
    }
    delete liveActivity.value[projectAgentKey(projectId, agent)]
  }

  function liveActivityOf(agent: Agent): TurnActivity | undefined {
    const project = projects.selected
    return project === null ? undefined : liveActivity.value[projectAgentKey(project.id, agent)]
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

  async function decidePermission(id: string, allow: boolean, always = false, reason?: string): Promise<void> {
    try {
      await answerPermission(id, allow, always, reason)
    } catch (error) {
      feed.value.push({ type: 'error', message: String(error) })
      permissions.value = permissions.value.filter((permission) => permission.id !== id)
    }
  }

  function notify(text: string): void {
    feed.value.push({ type: 'notice', text, time: Date.now() })
  }

  function showScreen(next: ConversationScreen): void {
    screen.value = next
  }

  // Trocar de projeto troca a conversa inteira: o feed em memória é do projeto anterior até recarregar do banco.
  watch(
    () => projects.selected?.id,
    (projectId, previous) => {
      showScreen(GENERAL_SCREEN)
      if (previous !== undefined && projectId !== undefined) void restoreHistory()
    },
  )

  return {
    feed,
    states,
    stateOf,
    thinkingAgent,
    pendingAgent,
    pendingSince,
    liveActivityOf,
    activeAgents,
    timeline,
    screen,
    showScreen,
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
