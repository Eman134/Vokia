import { invoke } from '@tauri-apps/api/core'
import type { OrchPlacement, OrchPlacementDraft, OrchTeam, OrchTeamDraft, OrchestrationSnapshot } from './orchestrationTypes'

const GET_ORCHESTRATION_COMMAND = 'get_orchestration'
const PLACE_AGENT_COMMAND = 'place_agent'
const SAVE_TEAM_COMMAND = 'save_team'
const DELETE_TEAM_COMMAND = 'delete_team'

export function fetchOrchestration(projectId: number): Promise<OrchestrationSnapshot> {
  return invoke<OrchestrationSnapshot>(GET_ORCHESTRATION_COMMAND, { projectId })
}

export function placeAgent(projectId: number, draft: OrchPlacementDraft): Promise<OrchPlacement> {
  return invoke<OrchPlacement>(PLACE_AGENT_COMMAND, { projectId, draft })
}

// Criar (id nulo) e renomear (id do time) usam o mesmo comando.
export function saveTeam(projectId: number, draft: OrchTeamDraft): Promise<OrchTeam> {
  return invoke<OrchTeam>(SAVE_TEAM_COMMAND, { projectId, draft })
}

export function deleteTeam(projectId: number, id: string): Promise<void> {
  return invoke<void>(DELETE_TEAM_COMMAND, { projectId, id })
}
