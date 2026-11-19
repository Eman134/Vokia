import { invoke } from '@tauri-apps/api/core'
import type { Agent } from '../team/events'

const LIST_AGENT_ACTIVITY_COMMAND = 'list_agent_activity'

// Uma execução em andamento: o próprio agente (cópia 0) ou uma réplica (cópia n), com o card que ela atende.
export interface AgentRun {
  instanceId: string
  copyNumber: number
  projectId: number
  projectName: string
  itemId: string | null
  itemTitle: string | null
}

export interface AgentActivity {
  agent: Agent
  runs: AgentRun[]
}

export function fetchAgentActivity(): Promise<AgentActivity[]> {
  return invoke<AgentActivity[]>(LIST_AGENT_ACTIVITY_COMMAND)
}
