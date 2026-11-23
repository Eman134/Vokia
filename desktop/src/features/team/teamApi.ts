import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  VOKIA_EVENT,
  type Agent,
  type CliProblem,
  type OverrideReason,
  type TemporaryOverride,
  type VokiaEvent,
} from './events'

export type { OverrideReason, TemporaryOverride, UnlistenFn }

const CLI_PROBLEM_COMMAND = 'cli_problem'
const SEND_REQUEST_COMMAND = 'send_request'
const ANSWER_PERMISSION_COMMAND = 'answer_permission'
const ANSWER_DECISION_COMMAND = 'answer_decision'
const CHECK_CONNECTION_COMMAND = 'check_connection'
const LIST_AGENT_PAUSES_COMMAND = 'list_agent_pauses'
const SET_AGENT_PAUSED_COMMAND = 'set_agent_paused'
const LIST_AGENTS_COMMAND = 'list_agents'
const AVAILABLE_PROVIDERS_COMMAND = 'available_providers'
const CREATE_AGENT_COMMAND = 'create_agent'
const UPDATE_AGENT_COMMAND = 'update_agent'
const DELETE_AGENT_COMMAND = 'delete_agent'
const SET_AGENT_INSTRUCTIONS_COMMAND = 'set_agent_instructions'
const LIST_TEMPORARY_OVERRIDES_COMMAND = 'list_temporary_overrides'
const SET_TEMPORARY_OVERRIDE_COMMAND = 'set_temporary_override'
const CLEAR_TEMPORARY_OVERRIDE_COMMAND = 'clear_temporary_override'
const CLEAR_ALL_TEMPORARY_OVERRIDES_COMMAND = 'clear_all_temporary_overrides'
const UPDATE_EMBER_LLM_COMMAND = 'update_ember_llm'
const LIST_INTERRUPTED_RUNS_COMMAND = 'list_interrupted_runs'
const RESUME_INTERRUPTED_RUN_COMMAND = 'resume_interrupted_run'
const DISCARD_INTERRUPTED_RUN_COMMAND = 'discard_interrupted_run'
const LIST_QUEUED_REQUESTS_COMMAND = 'list_queued_requests'
const ENQUEUE_QUEUED_REQUEST_COMMAND = 'enqueue_queued_request'
const UPDATE_QUEUED_REQUEST_COMMAND = 'update_queued_request'
const MOVE_QUEUED_REQUEST_COMMAND = 'move_queued_request'
const REMOVE_QUEUED_REQUEST_COMMAND = 'remove_queued_request'

export interface QueuedRequestRecord {
  id: number
  projectId: number
  to: Agent
  text: string
  channelId: number | null
  itemId: string | null
  attachments: string[]
  position: number
}

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

export type ModelTier = 'basic' | 'intermediate' | 'advanced'
export type ToolTransport = 'none' | 'mcp-native' | 'vokia-cli'

export interface ModelTiers {
  basic: string
  intermediate: string
  advanced: string
}

export interface LlmTarget {
  cli: string
  modelTier?: ModelTier | null
  model?: string | null
}

export interface AgentProfile {
  id: Agent
  name: string
  role: string
  avatar: string
  cli: string
  model: string
  modelTier?: ModelTier | null
  command: string
  builtin: boolean
  extraInstructions: string
  followDefaultLlm: boolean
  fallback: LlmTarget[]
}

export interface CliPreset {
  id: string
  label: string
  models: string[]
  defaultModel: string
  defaultCommand: string
  tiers: ModelTiers
  supportsMcp: boolean
  toolTransport?: ToolTransport
  supportsSandbox: boolean
  builtinSeed: boolean
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
  modelTier?: ModelTier | null
  command: string
  followDefaultLlm: boolean
  fallback: LlmTarget[]
}

export function fetchCliProblem(): Promise<CliProblem | null> {
  return invoke<CliProblem | null>(CLI_PROBLEM_COMMAND)
}

export type RequestOutcome = 'finished' | 'failed' | 'queued' | 'paused'

export function sendRequest(
  text: string,
  to: Agent,
  projectId: number,
  runId?: number,
  itemId?: string,
  channelId?: number,
  attachments?: string[],
): Promise<RequestOutcome> {
  return invoke<RequestOutcome>(SEND_REQUEST_COMMAND, { text, to, projectId, runId, itemId, channelId, attachments })
}

export function answerPermission(
  id: string,
  allow: boolean,
  always: boolean,
  reason?: string,
  root = false,
): Promise<void> {
  return invoke<void>(ANSWER_PERMISSION_COMMAND, { id, allow, always, reason: reason ?? null, root })
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

export function listQueuedRequests(): Promise<QueuedRequestRecord[]> {
  return invoke<QueuedRequestRecord[]>(LIST_QUEUED_REQUESTS_COMMAND)
}

export function enqueueQueuedRequest(params: {
  projectId: number
  to: Agent
  text: string
  channelId?: number
  itemId?: string
  attachments?: string[]
}): Promise<QueuedRequestRecord> {
  return invoke<QueuedRequestRecord>(ENQUEUE_QUEUED_REQUEST_COMMAND, params)
}

export function updateQueuedRequest(id: number, text: string): Promise<boolean> {
  return invoke<boolean>(UPDATE_QUEUED_REQUEST_COMMAND, { id, text })
}

export function moveQueuedRequest(id: number, direction: 'up' | 'down'): Promise<boolean> {
  return invoke<boolean>(MOVE_QUEUED_REQUEST_COMMAND, { id, direction })
}

export function removeQueuedRequest(id: number): Promise<boolean> {
  return invoke<boolean>(REMOVE_QUEUED_REQUEST_COMMAND, { id })
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

export function availableProviders(): Promise<string[]> {
  return invoke<string[]>(AVAILABLE_PROVIDERS_COMMAND)
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

export function listTemporaryOverrides(): Promise<TemporaryOverride[]> {
  return invoke<TemporaryOverride[]>(LIST_TEMPORARY_OVERRIDES_COMMAND)
}

export function setTemporaryOverride(params: {
  agent: Agent
  cli: string
  modelTier?: ModelTier | null
  model?: string | null
  reason?: OverrideReason | null
  expiresInMs?: number | null
}): Promise<TemporaryOverride> {
  return invoke<TemporaryOverride>(SET_TEMPORARY_OVERRIDE_COMMAND, {
    agent: params.agent,
    cli: params.cli,
    modelTier: params.modelTier ?? null,
    model: params.model ?? null,
    reason: params.reason ?? null,
    expiresInMs: params.expiresInMs ?? null,
  })
}

export function clearTemporaryOverride(agent: Agent): Promise<void> {
  return invoke<void>(CLEAR_TEMPORARY_OVERRIDE_COMMAND, { agent })
}

export function clearAllTemporaryOverrides(): Promise<void> {
  return invoke<void>(CLEAR_ALL_TEMPORARY_OVERRIDES_COMMAND)
}

export function updateEmberLlm(params: {
  cli?: string | null
  modelTier?: ModelTier | null
  model?: string | null
  fallback?: LlmTarget[] | null
}): Promise<AgentProfile> {
  return invoke<AgentProfile>(UPDATE_EMBER_LLM_COMMAND, {
    cli: params.cli ?? null,
    modelTier: params.modelTier ?? null,
    model: params.model ?? null,
    fallback: params.fallback ?? null,
  })
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
