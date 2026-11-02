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

export interface AgentPause {
  projectId: number
  agent: Agent
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
): Promise<RequestOutcome> {
  return invoke<RequestOutcome>(SEND_REQUEST_COMMAND, { text, to, projectId, runId })
}

export function answerPermission(id: string, allow: boolean): Promise<void> {
  return invoke<void>(ANSWER_PERMISSION_COMMAND, { id, allow })
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
