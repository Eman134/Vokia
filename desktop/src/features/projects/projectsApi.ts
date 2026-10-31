import { invoke } from '@tauri-apps/api/core'
import type { VokiaMessage } from '../team/events'

const DB_PROBLEM_COMMAND = 'db_problem'
const LIST_PROJECTS_COMMAND = 'list_projects'
const LOAD_HISTORY_COMMAND = 'load_history'

export interface Project {
  id: number
  name: string
  path: string
  selected: boolean
  createdAt: number
}

export interface DbProblem {
  reason: string
}

export function fetchDbProblem(): Promise<DbProblem | null> {
  return invoke<DbProblem | null>(DB_PROBLEM_COMMAND)
}

export function listProjects(): Promise<Project[]> {
  return invoke<Project[]>(LIST_PROJECTS_COMMAND)
}

export function loadHistory(projectId: number): Promise<VokiaMessage[]> {
  return invoke<VokiaMessage[]>(LOAD_HISTORY_COMMAND, { projectId })
}
