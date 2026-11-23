import { invoke } from '@tauri-apps/api/core'
import type { Agent } from './events'

const LIST_AGENT_COPIES_COMMAND = 'list_agent_copies'

export interface AgentCopy {
  id: string
  number: number
  itemId: string | null
  itemTitle: string | null
  worktreePath: string | null
}

export interface AgentCopies {
  agent: Agent
  copies: AgentCopy[]
}

export const SAME_FOLDER_TEXT = 'mesma pasta do projeto'
const PATH_SEPARATOR = /[\\/]/
const REPLICA_ID_PATTERN = /^(.+)-(\d+)$/

export function replicaOf(id: string): { base: string; number: number } | null {
  const match = REPLICA_ID_PATTERN.exec(id)
  if (!match) return null
  return { base: match[1]!, number: Number(match[2]) }
}

export function fetchAgentCopies(projectId: number): Promise<AgentCopies[]> {
  return invoke<AgentCopies[]>(LIST_AGENT_COPIES_COMMAND, { projectId })
}

export function worktreeFolder(path: string): string {
  return path.split(PATH_SEPARATOR).filter(Boolean).pop() ?? path
}
