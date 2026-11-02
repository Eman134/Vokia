import { invoke } from '@tauri-apps/api/core'
import type { Agent } from '../team/events'
import type { BacklogColumn, BacklogItem } from './backlogTypes'
import type { LinkedPlan } from './useItemPlan'

const LIST_BACKLOG_COMMAND = 'list_backlog'
const SAVE_BACKLOG_ITEM_COMMAND = 'save_backlog_item'
const MOVE_BACKLOG_ITEM_COMMAND = 'move_backlog_item'
const SAVE_ITEM_PLAN_COMMAND = 'save_item_plan'

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
}

export function fetchBacklog(projectId: number): Promise<BacklogSnapshot> {
  return invoke<BacklogSnapshot>(LIST_BACKLOG_COMMAND, { projectId })
}

export function saveBacklogItem(projectId: number, item: BacklogItem): Promise<void> {
  return invoke<void>(SAVE_BACKLOG_ITEM_COMMAND, { projectId, item })
}

export function moveBacklogItem(projectId: number, id: string, column: BacklogColumn): Promise<void> {
  return invoke<void>(MOVE_BACKLOG_ITEM_COMMAND, { projectId, id, column })
}

export function saveItemPlan(projectId: number, itemId: string, plan: ItemPlanInput): Promise<void> {
  return invoke<void>(SAVE_ITEM_PLAN_COMMAND, { projectId, itemId, plan })
}
