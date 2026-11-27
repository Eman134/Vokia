import { invoke } from '@tauri-apps/api/core'
import type { Agent, RequestProposal } from '../team/events'
import type { BacklogColumn, BacklogItem, NewRequestDraft, Sprint, SprintDraft, SprintPatch } from './backlogTypes'
import type { LinkedPlan } from './useItemPlan'

const LIST_BACKLOG_COMMAND = 'list_backlog'
const SAVE_BACKLOG_ITEM_COMMAND = 'save_backlog_item'
const CREATE_BACKLOG_ITEM_COMMAND = 'create_backlog_item'
const CREATE_REQUEST_FROM_PROPOSAL_COMMAND = 'create_request_from_proposal'
const MOVE_BACKLOG_ITEM_COMMAND = 'move_backlog_item'
const REORDER_BACKLOG_COLUMN_COMMAND = 'reorder_backlog_column'
const RETURN_TASK_COMMAND = 'return_task'
const STOP_TASK_RUN_COMMAND = 'stop_task_run'
const SAVE_ITEM_PLAN_COMMAND = 'save_item_plan'
const ARCHIVE_BACKLOG_ITEM_COMMAND = 'archive_backlog_item'
const DELETE_BACKLOG_ITEM_COMMAND = 'delete_backlog_item'
const LIST_SPRINTS_COMMAND = 'list_sprints'
const CREATE_SPRINT_COMMAND = 'create_sprint'
const UPDATE_SPRINT_COMMAND = 'update_sprint'
const CLOSE_SPRINT_COMMAND = 'close_sprint'
const DELETE_SPRINT_COMMAND = 'delete_sprint'
const SET_ITEM_SPRINT_COMMAND = 'set_item_sprint'

export const EMPTY_PLAN_STATE = 'vazio'

export type SavedPlanState = 'proposto' | 'vinculado'

export interface ItemPlanEntry {
  itemId: string
  state: SavedPlanState
  proposal?: string
  plan?: LinkedPlan
  agent?: Agent
  linkedAt: number
}

export interface ItemPlanInput {
  state: SavedPlanState | typeof EMPTY_PLAN_STATE
  proposal?: string
  plan?: LinkedPlan
  agent?: Agent
}

export interface BacklogSnapshot {
  items: BacklogItem[]
  plans: ItemPlanEntry[]
  sprints: Sprint[]
}

export function fetchBacklog(projectId: number): Promise<BacklogSnapshot> {
  return invoke<BacklogSnapshot>(LIST_BACKLOG_COMMAND, { projectId })
}

export function saveBacklogItem(projectId: number, item: BacklogItem): Promise<void> {
  return invoke<void>(SAVE_BACKLOG_ITEM_COMMAND, { projectId, item })
}

export interface NewBacklogRequest {
  title: string
  description?: string
  origin: NewRequestDraft['origin']
  links: NewRequestDraft['links']
}

export function createBacklogItem(projectId: number, request: NewBacklogRequest): Promise<BacklogItem> {
  return invoke<BacklogItem>(CREATE_BACKLOG_ITEM_COMMAND, { projectId, request })
}

export async function createRequestFromProposal(
  projectId: number,
  agent: Agent,
  proposal: RequestProposal,
): Promise<void> {
  await invoke(CREATE_REQUEST_FROM_PROPOSAL_COMMAND, {
    projectId,
    agent,
    title: proposal.title,
    detail: proposal.detail || null,
  })
}

export function moveBacklogItem(projectId: number, id: string, column: BacklogColumn): Promise<void> {
  return invoke<void>(MOVE_BACKLOG_ITEM_COMMAND, { projectId, id, column })
}

export function reorderBacklogColumn(
  projectId: number,
  column: BacklogColumn,
  itemIds: string[],
): Promise<void> {
  return invoke<void>(REORDER_BACKLOG_COLUMN_COMMAND, { projectId, column, itemIds })
}

// Devolve a task concluída ao líder: o motivo é obrigatório e o card volta a In progress.
export function returnTask(projectId: number, itemId: string, motive: string): Promise<void> {
  return invoke<void>(RETURN_TASK_COMMAND, { projectId, itemId, motive })
}

export function stopTaskRun(projectId: number, itemId: string): Promise<boolean> {
  return invoke<boolean>(STOP_TASK_RUN_COMMAND, { projectId, itemId })
}

export function saveItemPlan(projectId: number, itemId: string, plan: ItemPlanInput): Promise<void> {
  return invoke<void>(SAVE_ITEM_PLAN_COMMAND, { projectId, itemId, plan })
}

export function archiveBacklogItem(projectId: number, itemId: string): Promise<void> {
  return invoke<void>(ARCHIVE_BACKLOG_ITEM_COMMAND, { projectId, itemId })
}

export function deleteBacklogItem(projectId: number, itemId: string, cascade = false): Promise<void> {
  return invoke<void>(DELETE_BACKLOG_ITEM_COMMAND, { projectId, itemId, cascade })
}

export function listSprints(projectId: number): Promise<Sprint[]> {
  return invoke<Sprint[]>(LIST_SPRINTS_COMMAND, { projectId })
}

export function createSprint(projectId: number, sprint: SprintDraft): Promise<Sprint> {
  return invoke<Sprint>(CREATE_SPRINT_COMMAND, { projectId, sprint })
}

export function updateSprint(projectId: number, id: string, patch: SprintPatch): Promise<Sprint> {
  return invoke<Sprint>(UPDATE_SPRINT_COMMAND, { projectId, id, patch })
}

export function closeSprint(
  projectId: number,
  id: string,
  movePendingToSprintId?: string | null,
): Promise<Sprint> {
  return invoke<Sprint>(CLOSE_SPRINT_COMMAND, {
    projectId,
    id,
    movePendingToSprintId: movePendingToSprintId ?? null,
  })
}

export function deleteSprint(projectId: number, id: string): Promise<void> {
  return invoke<void>(DELETE_SPRINT_COMMAND, { projectId, id })
}

export function setItemSprint(projectId: number, itemId: string, sprintId: string | null): Promise<void> {
  return invoke<void>(SET_ITEM_SPRINT_COMMAND, { projectId, itemId, sprintId })
}
