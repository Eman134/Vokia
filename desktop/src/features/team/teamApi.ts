import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { VOKIA_EVENT, type Agent, type CliProblem, type VokiaEvent } from './events'

export type { UnlistenFn }

const CLI_PROBLEM_COMMAND = 'cli_problem'
const SEND_REQUEST_COMMAND = 'send_request'
const ANSWER_PERMISSION_COMMAND = 'answer_permission'
const ANSWER_DECISION_COMMAND = 'answer_decision'
const CHECK_CONNECTION_COMMAND = 'check_connection'
const LIST_AGENT_PAUSES_COMMAND = 'list_agent_pauses'
const SET_AGENT_PAUSED_COMMAND = 'set_agent_paused'
const LIST_AGENTS_COMMAND = 'list_agents'
const CREATE_AGENT_COMMAND = 'create_agent'
const UPDATE_AGENT_COMMAND = 'update_agent'
const DELETE_AGENT_COMMAND = 'delete_agent'
const SET_AGENT_INSTRUCTIONS_COMMAND = 'set_agent_instructions'
const LIST_INTERRUPTED_RUNS_COMMAND = 'list_interrupted_runs'
const RESUME_INTERRUPTED_RUN_COMMAND = 'resume_interrupted_run'
const DISCARD_INTERRUPTED_RUN_COMMAND = 'discard_interrupted_run'

export interface AgentPause {
  projectId: number
  agent: Agent
}

export interface InterruptedRun {
  id: number
  projectId: number
  agent: Agent
  request: string
  runId: number | null
  itemId: string | null
  startedAt: number
  leader: boolean
}

export interface AgentProfile {
  id: Agent
  name: string
  role: string
  avatar: string
  cli: string
  model: string
  builtin: boolean
  extraInstructions: string
}

export interface CliPreset {
  id: string
  label: string
  models: string[]
  defaultModel: string
}

export interface TeamListing {
  agents: AgentProfile[]
  presets: CliPreset[]
}

export interface AgentDraft {
  name: string
  role: string
  avatar: string
  cli: string
  model: string
}

export function fetchCliProblem(): Promise<CliProblem | null> {
  return invoke<CliProblem | null>(CLI_PROBLEM_COMMAND)
}

export type RequestOutcome = 'finished' | 'failed' | 'queued'

export function sendRequest(
  text: string,
  to: Agent,
  projectId: number,
  runId?: number,
  itemId?: string,
  channelId?: number,
): Promise<RequestOutcome> {
  return invoke<RequestOutcome>(SEND_REQUEST_COMMAND, { text, to, projectId, runId, itemId, channelId })
}

export function answerPermission(id: string, allow: boolean, always: boolean): Promise<void> {
  return invoke<void>(ANSWER_PERMISSION_COMMAND, { id, allow, always })
}

export function answerDecision(id: number, answer: string): Promise<void> {
  return invoke<void>(ANSWER_DECISION_COMMAND, { id, answer })
}

export function listAgentPauses(): Promise<AgentPause[]> {
  return invoke<AgentPause[]>(LIST_AGENT_PAUSES_COMMAND)
}

export function setAgentPaused(projectId: number, agent: Agent, paused: boolean): Promise<void> {
  return invoke<void>(SET_AGENT_PAUSED_COMMAND, { projectId, agent, paused })
}

export function listInterruptedRuns(): Promise<InterruptedRun[]> {
  return invoke<InterruptedRun[]>(LIST_INTERRUPTED_RUNS_COMMAND)
}

export function resumeInterruptedRun(id: number): Promise<RequestOutcome> {
  return invoke<RequestOutcome>(RESUME_INTERRUPTED_RUN_COMMAND, { id })
}

export function discardInterruptedRun(id: number): Promise<void> {
  return invoke<void>(DISCARD_INTERRUPTED_RUN_COMMAND, { id })
}

export function listAgents(): Promise<TeamListing> {
  return invoke<TeamListing>(LIST_AGENTS_COMMAND)
}

export function createAgent(draft: AgentDraft): Promise<AgentProfile> {
  return invoke<AgentProfile>(CREATE_AGENT_COMMAND, { draft })
}

export function updateAgent(id: Agent, draft: AgentDraft): Promise<AgentProfile> {
  return invoke<AgentProfile>(UPDATE_AGENT_COMMAND, { id, draft })
}

export function setAgentInstructions(id: Agent, text: string): Promise<AgentProfile> {
  return invoke<AgentProfile>(SET_AGENT_INSTRUCTIONS_COMMAND, { id, text })
}

export function deleteAgent(id: Agent): Promise<void> {
  return invoke<void>(DELETE_AGENT_COMMAND, { id })
}

export function listenTeamEvents(onEvent: (event: VokiaEvent) => void): Promise<UnlistenFn> {
  return listen<VokiaEvent>(VOKIA_EVENT, (event) => onEvent(event.payload))
}

export function checkConnection(): Promise<boolean> {
  // Offline é estado, não erro: uma exceção do comando conta como falso.
  return invoke<boolean>(CHECK_CONNECTION_COMMAND).catch(() => false)
}

export function listenConnectionChanges(onOnline: () => void, onOffline: () => void): () => void {
  window.addEventListener('online', onOnline)
  window.addEventListener('offline', onOffline)
  return () => {
    window.removeEventListener('online', onOnline)
    window.removeEventListener('offline', onOffline)
  }
}
