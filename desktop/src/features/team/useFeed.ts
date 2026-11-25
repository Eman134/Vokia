import { useExecutionMode } from '../permissions/useExecutionMode'
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
import { replicaOf } from './agentCopies'
import { isPendingState, selectPendingAgents } from './pendingAgents'
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
import { useArtifacts } from '../artifacts/useArtifacts'
import { useRequests } from './useRequests'
import { useOrchestration } from '../orchestration/useOrchestration'
import { useAgentActivity } from '../teammap/useAgentActivity'
import { useUsageLimit } from '../usage/useUsageLimit'
import { useTemporaryOverrides } from '../providers/useTemporaryOverrides'
import { useActiveLlm } from '../providers/useActiveLlm'

const GENERAL_SCREEN: ConversationScreen = { kind: 'channel', channel: GENERAL_CHANNEL_NAME }

export const useFeed = defineStore('feed', () => {
  const feed = ref<FeedEntry[]>([])
  const roster = useRoster()
  const states = ref<Record<Agent, AgentState>>({})
  const stateSince = ref<Partial<Record<Agent, number>>>({})
  const stateItem = ref<Record<Agent, string | undefined>>({})
  const liveActivity = ref<Record<string, TurnActivity>>({})
  const liveActivityItem = ref<Record<string, string | undefined>>({})
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
  function itemOf(agent: Agent): string | undefined {
    return stateItem.value[agent]
  }
  function activeAgentsForItems(itemIds: ReadonlySet<string>): Set<Agent> {
    const agents = new Set<Agent>()
    for (const [agent, agentItemId] of Object.entries(stateItem.value)) {
      if (agentItemId !== undefined && itemIds.has(agentItemId) && stateOf(agent) !== 'idle') {
        agents.add(agent)
      }
    }
    return agents
  }
  function profileForPending(id: Agent): AgentProfile {
    const existing = roster.profileOf(id)
    if (existing) return existing
    const replica = replicaOf(id)
    const base = replica ? roster.profileOf(replica.base) : undefined
    return {
      id,
      name: roster.nameOf(id),
      role: base?.role ?? '',
      avatar: roster.avatarOf(id),
      cli: base?.cli ?? '',
      model: base?.model ?? '',
      command: base?.command ?? '',
      builtin: base?.builtin ?? false,
      extraInstructions: base?.extraInstructions ?? '',
      followDefaultLlm: base?.followDefaultLlm ?? true,
      fallback: base?.fallback ?? [],
    }
  }
  const pendingAgents = computed<AgentProfile[]>(() => {
    const known = selectPendingAgents(roster.agents, states.value)
    const knownIds = new Set(roster.agents.map((agent) => agent.id))
    const extras = Object.entries(states.value)
      .filter(([id, state]) => !knownIds.has(id) && isPendingState(state))
      .map(([id]) => profileForPending(id))
    return [...known, ...extras]
  })
  function sinceOf(agent: Agent): number | undefined {
    return stateSince.value[agent]
  }
  const activeAgents = computed<AgentProfile[]>(() => selectActiveAgents(roster.agents, states.value))
  const channels = useChannels()
  const timeline = computed<TimelineItem[]>(() => {
    const current = screen.value
    if (current.kind === 'triage') {
      const triagingIds = useBacklog().triagingIds
      return buildTimeline(screenEntries(feed.value, current, channels.channels, activeAgentsForItems(triagingIds), triagingIds))
    }
    const threadAgents = current.kind === 'thread' ? activeAgentsForItems(new Set([current.itemId])) : undefined
    return buildTimeline(screenEntries(feed.value, current, channels.channels, threadAgents))
  })

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
      await useRequests().load()
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
        if (event.isError && event.from !== 'user' && event.text === 'Limite esgotado') {
          useUsageLimit().suggestFromLimitMessage(event.from)
        }
        break
      case 'error':
        feed.value.push(event)
        break
      case 'agent-activity': {
        const key = projectAgentKey(event.projectId, event.agent)
        liveActivity.value[key] = event.activity
        if (event.itemId !== undefined) {
          liveActivityItem.value[key] = event.itemId
        } else {
          delete liveActivityItem.value[key]
        }
        break
      }
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
          outcome: event.mode ?? 'project-rule',
          time: Date.now(),
        })
        break
      case 'execution-mode-changed':
        useExecutionMode().apply(event)
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
      case 'artifacts-changed':
        void useArtifacts().reloadIfCurrent(event.projectId)
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
      case 'task-stage':
        useOrchestration().applyTaskStage(event.stage)
        break
      case 'plan-usage':
        break
      case 'usage-limit':
        useUsageLimit().apply(event)
        break
      case 'usage-limit-failed':
        if (event.projectId === projects.selected?.id) {
          feed.value.push({ ...event, time: Date.now() })
        }
        break
      case 'agent-pauses-changed':
        control.applyPauses(event.pauses)
        break
      case 'model-unavailable':
        useUsageLimit().suggestModelUnavailable(event)
        break
      case 'temporary-overrides-changed':
        useTemporaryOverrides().apply(event.overrides)
        useActiveLlm().syncOverrides(event.overrides)
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

  function recordState({ agent, state, itemId }: VokiaAgentState): void {
    if (stateOf(agent) !== state) {
      stateSince.value[agent] = Date.now()
    }
    states.value[agent] = state
    if (state === 'idle' || itemId === undefined) {
      delete stateItem.value[agent]
    } else {
      stateItem.value[agent] = itemId
    }
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
    const key = projectAgentKey(projectId, agent)
    delete liveActivity.value[key]
    delete liveActivityItem.value[key]
  }

  function liveActivityOf(agent: Agent, itemId?: string): TurnActivity | undefined {
    const project = projects.selected
    if (project === null) return undefined
    const key = projectAgentKey(project.id, agent)
    if (itemId !== undefined && liveActivityItem.value[key] !== itemId) {
      return undefined
    }
    return liveActivity.value[key]
  }

  function stepLabelOf(agent: Agent, projectId?: number): string | undefined {
    const resolvedProjectId = projectId ?? projects.selected?.id ?? lastProject.value[agent] ?? null
    if (resolvedProjectId === null) return undefined
    const label = liveActivity.value[projectAgentKey(resolvedProjectId, agent)]?.currentStepLabel?.trim()
    return label ? label : undefined
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

  async function decidePermission(
    id: string,
    allow: boolean,
    always = false,
    reason?: string,
    root = false,
  ): Promise<void> {
    try {
      await answerPermission(id, allow, always, reason, root)
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
    stateSince,
    stateItem,
    stateOf,
    itemOf,
    activeAgentsForItems,
    sinceOf,
    thinkingAgent,
    pendingAgent,
    pendingSince,
    pendingAgents,
    liveActivityOf,
    stepLabelOf,
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
