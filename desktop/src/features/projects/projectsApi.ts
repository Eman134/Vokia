import { invoke } from '@tauri-apps/api/core'
import type { VokiaMessage } from '../team/events'

const DB_PROBLEM_COMMAND = 'db_problem'
const LIST_PROJECTS_COMMAND = 'list_projects'
const LOAD_HISTORY_COMMAND = 'load_history'
const SELECT_PROJECT_COMMAND = 'select_project'
const PICK_FOLDER_COMMAND = 'pick_folder'
const INSPECT_FOLDER_COMMAND = 'inspect_folder'
const INSPECT_GITHUB_REPO_COMMAND = 'inspect_github_repo'
const CREATE_PROJECT_COMMAND = 'create_project'

export type OriginKind = 'folder' | 'github'
export type CloneMode = 'link' | 'fork'

export interface Project {
  id: number
  name: string
  path: string
  selected: boolean
  createdAt: number
  symbol: string
  description: string
  originKind: OriginKind
  originRepo: string | null
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

export type ProjectErrorCode =
  | 'GitNotInstalled' | 'FolderNotFound' | 'NotADirectory'  | 'GhNotInstalled' | 'GhNotAuthenticated' | 'InvalidRepoRef' | 'RepoNotFound' | 'GhFailed'
  | 'DestinationExists' | 'CloneFailed' | 'CloneTimeout' | 'BranchNotFound'
  | 'ProjectAlreadyAdded' | 'CreateInProgress' | 'InvalidIcon' | 'Db'

export interface ProjectError {
  code: ProjectErrorCode
  message: string
  path?: string
  repo?: string
  branch?: string
}

export interface FolderInfo {
  path: string
  name: string
  branch: string | null
  remoteUrl: string | null
}

export interface GithubRepoInfo {
  fullName: string
  name: string
  defaultBranch: string | null
  isFork: boolean
  url: string
}

export type NewProjectOrigin =
  | { kind: 'folder'; path: string }
  | { kind: 'github'; repo: string; mode: CloneMode; parentDir: string; branch: string | null }

export interface NewProjectInput {
  name: string
  symbol: string
  description: string
  origin: NewProjectOrigin
}

export function isProjectError(error: unknown): error is ProjectError {
  return typeof error === 'object' && error !== null && 'code' in error && 'message' in error
}

export function selectProject(id: number): Promise<void> {
  return invoke<void>(SELECT_PROJECT_COMMAND, { id })
}

export function pickFolder(): Promise<string | null> {
  return invoke<string | null>(PICK_FOLDER_COMMAND)
}

export function inspectFolder(path: string): Promise<FolderInfo> {
  return invoke<FolderInfo>(INSPECT_FOLDER_COMMAND, { path })
}

export function inspectGithubRepo(repo: string): Promise<GithubRepoInfo> {
  return invoke<GithubRepoInfo>(INSPECT_GITHUB_REPO_COMMAND, { repo })
}

export function createProject(input: NewProjectInput): Promise<Project> {
  return invoke<Project>(CREATE_PROJECT_COMMAND, { input })
}
