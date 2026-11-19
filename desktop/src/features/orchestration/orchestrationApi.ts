import { invoke } from '@tauri-apps/api/core'
import type { Agent } from '../team/events'
import type {
  FinalValidation,
  OrchPlacement,
  OrchPlacementDraft,
  OrchTeam,
  OrchTeamDraft,
  OrchestrationSnapshot,
  TeamFlowDraft,
} from './orchestrationTypes'

const GET_ORCHESTRATION_COMMAND = 'get_orchestration'
const PLACE_AGENT_COMMAND = 'place_agent'
const SAVE_TEAM_COMMAND = 'save_team'
const DELETE_TEAM_COMMAND = 'delete_team'
const SAVE_TEAM_FLOW_COMMAND = 'save_team_flow'
const SET_FINAL_VALIDATION_COMMAND = 'set_final_validation'

export function fetchOrchestration(): Promise<OrchestrationSnapshot> {
  return invoke<OrchestrationSnapshot>(GET_ORCHESTRATION_COMMAND)
}

export function placeAgent(draft: OrchPlacementDraft): Promise<OrchPlacement> {
  return invoke<OrchPlacement>(PLACE_AGENT_COMMAND, { draft })
}

// Criar (id nulo) e renomear (id do time) usam o mesmo comando.
// leaderAgent nulo é o líder "Automático": o backend limpa o líder gravado do time.
export function saveTeam(draft: OrchTeamDraft, leaderAgent: Agent | null): Promise<OrchTeam> {
  return invoke<OrchTeam>(SAVE_TEAM_COMMAND, { draft, leaderAgent })
}

export function deleteTeam(id: string): Promise<void> {
  return invoke<void>(DELETE_TEAM_COMMAND, { id })
}

// Única porta de escrita da estrutura do fluxo; recusado, o erro é a lista de FlowProblem em JSON.
export function saveTeamFlow(teamId: string, draft: TeamFlowDraft): Promise<OrchestrationSnapshot> {
  return invoke<OrchestrationSnapshot>(SAVE_TEAM_FLOW_COMMAND, { teamId, draft })
}

export function setFinalValidation(teamId: string, mode: FinalValidation): Promise<void> {
  return invoke<void>(SET_FINAL_VALIDATION_COMMAND, { teamId, mode })
}
