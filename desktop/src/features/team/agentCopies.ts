import { invoke } from '@tauri-apps/api/core'
import type { Agent } from './events'

const LIST_AGENT_COPIES_COMMAND = 'list_agent_copies'

export interface AgentCopy {
  id: string
  agent: Agent
  number: number
  itemId: string | null
  itemTitle: string | null
  worktreePath: string | null
}

export const SAME_FOLDER_TEXT = 'mesma pasta do projeto'
const PATH_SEPARATOR = /[\\/]/

export function fetchAgentCopies(projectId: number): Promise<AgentCopy[]> {
  return invoke<AgentCopy[]>(LIST_AGENT_COPIES_COMMAND, { projectId })
}

export function worktreeFolder(path: string): string {
  return path.split(PATH_SEPARATOR).filter(Boolean).pop() ?? path
}
