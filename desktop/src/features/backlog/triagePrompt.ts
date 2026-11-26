import type { OrchestrationSnapshot, OrchLayer } from '../orchestration/orchestrationTypes'
import type { AgentProfile } from '../team/teamApi'
import type { BacklogItem } from './backlogTypes'
import type { Role } from '../agents/rolesApi'
import { PLAN_PROTOCOL_HINT } from './planProposal'

const NO_TEAM_LABEL = 'Sem time'
const EMPTY_TEAM_TEXT = 'nenhum agente posicionado'
const ROLES_HEADING = 'Papéis dos agentes (descrição e instruções):'
const FLOW_HEADING = 'Fluxo de cada time (referência de handoff entre camadas, não obrigatório):'
const NO_FLOW_TEXT = 'sem arestas definidas'
const NO_LEADER_TEXT = 'sem líder'
const STAGE_RULE =
  'Ao propor o plano, monte as etapas pelo papel e pelas instruções de cada agente do time escolhido e use no #roteiro (quem:) apenas agentes desse time. Qualquer membro pode abrir o plano. Divida o trabalho em quantas etapas forem necessárias (um #lote por etapa); etapas independentes rodam em paralelo e uma que depende de outra declara apos:<n> no #roteiro.'
const DETAIL_INDENT = '    '

function memberLabel(profile: AgentProfile | undefined, agentId: string, layer: OrchLayer | undefined): string {
  const layerText = layer ? `camada ${layer.name}, tipo ${layer.kind}` : 'camada desconhecida'
  const role = profile?.role ? `, papel ${profile.role}` : ''
  return `${profile?.name ?? agentId} (${agentId}, ${layerText}${role})`
}

function instructionLines(label: string, text: string | undefined): string[] {
  const trimmed = text?.trim()
  return trimmed ? [`${DETAIL_INDENT}${label}: ${trimmed}`] : []
}

function roleLines(agents: readonly AgentProfile[], roles: readonly Role[]): string[] {
  const used = new Set(agents.map((agent) => agent.role))
  return roles
    .filter((role) => used.has(role.name))
    .flatMap((role) => {
      const text = [role.description, role.instructions].map((part) => part.trim()).filter(Boolean).join(' ')
      return text ? [`- ${role.name}: ${text}`] : []
    })
}

export function buildTriagePrompt(
  item: BacklogItem,
  snapshot: OrchestrationSnapshot,
  agents: readonly AgentProfile[],
  roles: readonly Role[],
): string {
  const layerOf = (layerId: string): OrchLayer | undefined => snapshot.layers.find((layer) => layer.id === layerId)
  const profileOf = (agentId: string): AgentProfile | undefined => agents.find((agent) => agent.id === agentId)
  const teamLines = snapshot.teams.flatMap((team) => {
    const members = snapshot.placements
      .filter((placement) => placement.teamId === team.id)
      .flatMap((placement) => {
        const profile = profileOf(placement.agentId)
        const layer = layerOf(placement.layerId)
        return [
          `  - ${memberLabel(profile, placement.agentId, layer)}`,
          ...instructionLines('instruções do agente', profile?.extraInstructions),
          ...instructionLines('instruções da camada', layer?.instructions),
          ...instructionLines('instruções do posicionamento', placement.instructions),
        ]
      })
    return [`- ${team.name}:`, ...instructionLines('instruções do time', team.instructions), ...(members.length ? members : [`  ${EMPTY_TEAM_TEXT}`])]
  })
  const flowLines = snapshot.teams.flatMap((team) => {
    const layers = snapshot.layers
      .filter((layer) => snapshot.placements.some((placement) => placement.teamId === team.id && placement.layerId === layer.id))
      .sort((a, b) => a.position - b.position)
      .map((layer) => layer.name)
    const nodeLabel = (node: string): string => profileOf(node)?.name ?? node
    const edges = snapshot.edges
      .filter((edge) => edge.teamId === team.id)
      .sort((a, b) => a.position - b.position)
      .map((edge) => `${nodeLabel(edge.fromNode)}→${nodeLabel(edge.toNode)} (${edge.syncMode}, ${edge.whenEvent})`)
    const leader = team.leaderAgent === null ? NO_LEADER_TEXT : `líder ${profileOf(team.leaderAgent)?.name ?? team.leaderAgent}`
    return [`- ${team.name} (${leader}): camadas ${layers.join(' → ') || EMPTY_TEAM_TEXT}; arestas ${edges.join('; ') || NO_FLOW_TEXT}`]
  })
  const placedIds = new Set(snapshot.placements.map((placement) => placement.agentId))
  const unplaced = agents.filter((agent) => !placedIds.has(agent.id))
  const unplacedLine = unplaced.length
    ? [`- ${NO_TEAM_LABEL}: ${unplaced.map((agent) => `${agent.name} (${agent.id})`).join('; ')}`]
    : []
  const roleSection = roleLines(agents, roles)
  const rolesBlock = roleSection.length ? ['', ROLES_HEADING, ...roleSection] : []
  const description = item.description ? [item.description] : []
  return [
    `Triagem do item ${item.id}: ${item.title}`,
    ...description,
    '',
    'O item está sem responsável. Decida quem executa:',
    `- Para atribuir direto, chame a ferramenta backlog_set_responsible com item_id "${item.id}" e agent com o id entre parênteses abaixo.`,
    '- Para delegar, use o fluxo de delegação do time. Você só delega: quem grava o responsável é a ferramenta.',
    `- Ao gravar plano via save_item_plan, use ${PLAN_PROTOCOL_HINT}`,
    '- Na triagem, defina também um public_id bem curto para o card (ex: FIL01 para filtros, ANIM01 para animações, máx. 8 caracteres) ao chamar save_item_plan ou backlog_set_responsible.',
    '',
    'Times e camadas deste projeto:',
    ...teamLines,
    ...unplacedLine,
    ...rolesBlock,
    '',
    FLOW_HEADING,
    ...flowLines,
    STAGE_RULE,
  ].join('\n')
}
