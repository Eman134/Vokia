export const VOKIA_EVENT = 'vokia://event'

export type Agent = 'iara' | 'davi'

export type Speaker = 'user' | Agent

export type AgentState = 'thinking' | 'waiting' | 'idle'

export interface VokiaMessage {
  type: 'message'
  from: Speaker
  to: Speaker
  text: string
  time: number
}

export interface VokiaAgentState {
  type: 'agent-state'
  agent: Agent
  state: AgentState
}

export interface VokiaError {
  type: 'error'
  message: string
}

export type VokiaEvent = VokiaMessage | VokiaAgentState | VokiaError

export interface CliProblem {
  reason: string
  fixCommand: string
}
