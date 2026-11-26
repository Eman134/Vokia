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
const UPDATE_PROJECT_COMMAND = 'update_project'
const DELETE_PROJECT_COMMAND = 'delete_project'
const CLEAR_CONVERSATION_COMMAND = 'clear_conversation'
const PIN_PROJECT_COMMAND = 'pin_project'
const SET_PERMISSION_LEVEL_COMMAND = 'set_permission_level'
const LIST_PERMISSION_RULES_COMMAND = 'list_permission_rules'
const DELETE_PERMISSION_RULE_COMMAND = 'delete_permission_rule'

export type OriginKind = 'folder' | 'github'
export type CloneMode = 'link' | 'fork'
export type PermissionLevel = 'liberado' | 'edicao' | 'leitura'

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
  pinned: boolean
  lastActivityAt: number | null
  permissionLevel: PermissionLevel
  color: string | null
  setupCommand: string | null
}

export const SETUP_COMMAND_MAX_CHARS = 300

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
  | 'ProjectAlreadyAdded' | 'CreateInProgress' | 'InvalidIcon' | 'InvalidColor' | 'ProjectNotFound' | 'ProjectBusy' | 'ProjectHasNoFolder' | 'Db'

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
  githubRepo: string | null
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
  color: string
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

export interface ProjectEdit {
  name: string
  symbol: string
  description: string
  color: string
  setupCommand: string
}

export function createProject(input: NewProjectInput): Promise<Project> {
  return invoke<Project>(CREATE_PROJECT_COMMAND, { input })
}

export function updateProject(id: number, input: ProjectEdit): Promise<Project> {
  return invoke<Project>(UPDATE_PROJECT_COMMAND, { id, input })
}

export function deleteProject(id: number): Promise<void> {
  return invoke<void>(DELETE_PROJECT_COMMAND, { id })
}

export interface ClearConversationScope {
  kind: 'all' | 'agent' | 'copy' | 'channel' | 'general' | 'thread' | 'triage'
  agent?: string
  channelId?: number
  itemId?: string
  itemIds?: string[]
}

export function clearConversation(projectId: number, scope?: ClearConversationScope): Promise<void> {
  return invoke<void>(CLEAR_CONVERSATION_COMMAND, { projectId, scope: scope ?? null })
}

export function pinProject(id: number | null): Promise<void> {
  return invoke<void>(PIN_PROJECT_COMMAND, { id })
}

export function setPermissionLevel(projectId: number, level: PermissionLevel): Promise<void> {
  return invoke<void>(SET_PERMISSION_LEVEL_COMMAND, { projectId, level })
}

export interface PermissionRule {
  tool: string
  command: string
  createdAt: number
}

export function listPermissionRules(projectId: number): Promise<PermissionRule[]> {
  return invoke<PermissionRule[]>(LIST_PERMISSION_RULES_COMMAND, { projectId })
}

export function deletePermissionRule(projectId: number, rule: PermissionRule): Promise<void> {
  return invoke<void>(DELETE_PERMISSION_RULE_COMMAND, { projectId, tool: rule.tool, command: rule.command })
}
