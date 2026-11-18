import { invoke } from '@tauri-apps/api/core'
import type { Agent } from '../team/events'
import type {
  OrchLayer,
  OrchLayerDraft,
  OrchMovement,
  OrchPlacement,
  OrchPlacementDraft,
  OrchRule,
  OrchRuleDraft,
  OrchTeam,
  OrchTeamDraft,
  OrchestrationSettings,
  OrchestrationSnapshot,
} from './orchestrationTypes'

const GET_ORCHESTRATION_COMMAND = 'get_orchestration'
const SAVE_LAYER_COMMAND = 'save_layer'
const DELETE_LAYER_COMMAND = 'delete_layer'
const REORDER_LAYERS_COMMAND = 'reorder_layers'
const SAVE_TEAM_COMMAND = 'save_team'
const DELETE_TEAM_COMMAND = 'delete_team'
const PLACE_AGENT_COMMAND = 'place_agent'
const UNPLACE_AGENT_COMMAND = 'unplace_agent'
const SAVE_RULE_COMMAND = 'save_rule'
const DELETE_RULE_COMMAND = 'delete_rule'
const SAVE_ORCHESTRATION_SETTINGS_COMMAND = 'save_orchestration_settings'
const LIST_MOVEMENTS_COMMAND = 'list_movements'

export function fetchOrchestration(projectId: number): Promise<OrchestrationSnapshot> {
  return invoke<OrchestrationSnapshot>(GET_ORCHESTRATION_COMMAND, { projectId })
}

export function saveLayer(projectId: number, draft: OrchLayerDraft): Promise<OrchLayer> {
  return invoke<OrchLayer>(SAVE_LAYER_COMMAND, { projectId, draft })
}

export function deleteLayer(projectId: number, id: string): Promise<void> {
  return invoke<void>(DELETE_LAYER_COMMAND, { projectId, id })
}

export function reorderLayers(projectId: number, ids: string[]): Promise<void> {
  return invoke<void>(REORDER_LAYERS_COMMAND, { projectId, ids })
}

export function saveTeam(projectId: number, draft: OrchTeamDraft): Promise<OrchTeam> {
  return invoke<OrchTeam>(SAVE_TEAM_COMMAND, { projectId, draft })
}

export function deleteTeam(projectId: number, id: string): Promise<void> {
  return invoke<void>(DELETE_TEAM_COMMAND, { projectId, id })
}

export function placeAgent(projectId: number, draft: OrchPlacementDraft): Promise<OrchPlacement> {
  return invoke<OrchPlacement>(PLACE_AGENT_COMMAND, { projectId, draft })
}

export function unplaceAgent(projectId: number, agentId: Agent): Promise<void> {
  return invoke<void>(UNPLACE_AGENT_COMMAND, { projectId, agentId })
}

export function saveRule(projectId: number, draft: OrchRuleDraft): Promise<OrchRule> {
  return invoke<OrchRule>(SAVE_RULE_COMMAND, { projectId, draft })
}

export function deleteRule(projectId: number, id: number): Promise<void> {
  return invoke<void>(DELETE_RULE_COMMAND, { projectId, id })
}

export function saveOrchestrationSettings(projectId: number, settings: OrchestrationSettings): Promise<void> {
  return invoke<void>(SAVE_ORCHESTRATION_SETTINGS_COMMAND, { projectId, settings })
}

export function fetchMovements(projectId: number): Promise<OrchMovement[]> {
  return invoke<OrchMovement[]>(LIST_MOVEMENTS_COMMAND, { projectId })
}
