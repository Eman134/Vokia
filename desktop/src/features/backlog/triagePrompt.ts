import type { OrchestrationSnapshot } from '../orchestration/orchestrationTypes'
import type { AgentProfile } from '../team/teamApi'
import type { BacklogItem } from './backlogTypes'
import { PLAN_PROTOCOL_HINT } from './planProposal'

const NO_TEAM_LABEL = 'Sem time'
const EMPTY_TEAM_TEXT = 'nenhum agente posicionado'

function memberLabel(profile: AgentProfile | undefined, agentId: string, layerName: string): string {
  return `${profile?.name ?? agentId} (${agentId}, camada ${layerName})`
}

export function buildTriagePrompt(
  item: BacklogItem,
  snapshot: OrchestrationSnapshot,
  agents: readonly AgentProfile[],
): string {
  const layerNameOf = (layerId: string): string => snapshot.layers.find((layer) => layer.id === layerId)?.name ?? layerId
  const profileOf = (agentId: string): AgentProfile | undefined => agents.find((agent) => agent.id === agentId)
  const teamLines = snapshot.teams.map((team) => {
    const members = snapshot.placements
      .filter((placement) => placement.teamId === team.id)
      .map((placement) => memberLabel(profileOf(placement.agentId), placement.agentId, layerNameOf(placement.layerId)))
    return `- ${team.name}: ${members.join('; ') || EMPTY_TEAM_TEXT}`
  })
  const placedIds = new Set(snapshot.placements.map((placement) => placement.agentId))
  const unplaced = agents.filter((agent) => !placedIds.has(agent.id))
  const unplacedLine = unplaced.length
    ? [`- ${NO_TEAM_LABEL}: ${unplaced.map((agent) => `${agent.name} (${agent.id})`).join('; ')}`]
    : []
  const description = item.description ? [item.description] : []
  return [
    `Triagem do item ${item.id}: ${item.title}`,
    ...description,
    '',
    'O item está sem responsável. Decida quem executa:',
    `- Para atribuir direto, chame a ferramenta backlog_set_responsible com item_id "${item.id}" e agent com o id entre parênteses abaixo.`,
    '- Para delegar, use o fluxo de delegação do time. Você só delega: quem grava o responsável é a ferramenta.',
    `- Ao gravar plano via save_item_plan, use ${PLAN_PROTOCOL_HINT}`,
    '',
    'Times e camadas deste projeto:',
    ...teamLines,
    ...unplacedLine,
  ].join('\n')
}
