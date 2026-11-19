import { clampEndpoint, clampNode, endLineKey, lineKey, START_LINE_KEY, type FlowEdge, type FlowPoint } from './flowGraph'
import type { ValidationInput } from './flowValidation'
import { DEFAULT_INSTANCES } from './orchestrationForm'
import {
  FLOW_END,
  FLOW_START,
  type FlowEdgeDraft,
  type FlowNodeDraft,
  type OrchFlowEdge,
  type OrchPlacementDraft,
  type OrchestrationSnapshot,
  type TeamFlowDraft,
} from './orchestrationTypes'
import {
  AGENT_META,
  COMMUNICATION_META,
  END_META,
  forgetFlowDraft,
  START_META,
  useFlowDraft,
  type FlowDraft,
  type FlowEdgeMeta,
  type FlowNodeConfig,
  type FlowSource,
} from './useFlowDraft'

// Sem curva gravada, a linha usa a curva padrão do canvas.
const NO_CURVE = 0

function metaOf(edge: OrchFlowEdge): FlowEdgeMeta {
  return { whenEvent: edge.whenEvent, action: edge.action, requestKind: edge.kind, turnLimit: edge.turnLimit, isDefault: edge.isDefault, instructions: edge.instructions }
}

function draftOf(edge: OrchFlowEdge): FlowEdgeDraft {
  return {
    fromNode: edge.fromNode,
    toNode: edge.toNode,
    link: edge.link,
    syncMode: edge.syncMode,
    whenEvent: edge.whenEvent,
    isDefault: edge.isDefault,
    action: edge.action,
    kind: edge.kind,
    turnLimit: edge.turnLimit,
    curve: edge.curve,
    instructions: edge.instructions,
  }
}

// O fluxo de um time como o backend o guarda: agentes do time, ligações entre eles, início, fim e configurações.
export function flowSourceFrom(snapshot: OrchestrationSnapshot, teamId: string): FlowSource {
  const team = snapshot.teams.find((candidate) => candidate.id === teamId)
  const placements = snapshot.placements.filter((placement) => placement.teamId === teamId)
  const agents = placements.map((placement) => placement.agentId)
  const gatewayIds = snapshot.gateways.filter((gateway) => gateway.teamId === teamId).map((gateway) => gateway.id)
  const known = new Set<string>([...agents, ...gatewayIds])
  const isAgent = new Set<string>(agents)
  const source: FlowSource = {
    agents,
    positions: new Map([
      ...placements.map((placement): [string, FlowPoint] => [placement.agentId, clampNode({ x: placement.posX, y: placement.posY })]),
      ...snapshot.gateways
        .filter((gateway) => gateway.teamId === teamId)
        .map((gateway): [string, FlowPoint] => [gateway.id, clampNode({ x: gateway.posX, y: gateway.posY })]),
    ]),
    edges: [],
    leader: null,
    endAgents: [],
    configs: new Map(
      placements.map((placement) => [
        placement.agentId,
        {
          instructions: placement.instructions,
          criteria: placement.criteria,
          instances: placement.maxInstances,
          worktrees: placement.maxWorktrees ?? 0,
        },
      ]),
    ),
    endpoints: {
      start: team?.startX != null && team.startY != null ? clampEndpoint({ x: team.startX, y: team.startY }) : null,
      end: team?.endX != null && team.endY != null ? clampEndpoint({ x: team.endX, y: team.endY }) : null,
    },
    curves: new Map(),
    metas: new Map(),
    gateways: snapshot.gateways
      .filter((gateway) => gateway.teamId === teamId)
      .map((gateway) => ({ id: gateway.id, kind: gateway.kind })),
    extraEdges: [],
  }
  const remember = (key: string, edge: OrchFlowEdge): void => {
    source.metas.set(key, metaOf(edge))
    if (edge.curve !== NO_CURVE) source.curves.set(key, edge.curve)
  }
  for (const edge of snapshot.edges.filter((candidate) => candidate.teamId === teamId)) {
    const fromNode = known.has(edge.fromNode)
    const toNode = known.has(edge.toNode)
    if (edge.link === 'fluxo' && edge.fromNode === FLOW_START && isAgent.has(edge.toNode) && source.leader === null) {
      source.leader = edge.toNode
      remember(START_LINE_KEY, edge)
    } else if (edge.link === 'fluxo' && edge.toNode === FLOW_END && fromNode) {
      source.endAgents.push(edge.fromNode)
      remember(endLineKey(edge.fromNode), edge)
    } else if ((edge.link === 'fluxo' || isAgent.has(edge.fromNode)) && fromNode && toNode) {
      const flowEdge: FlowEdge = {
        from: edge.fromNode,
        to: edge.toNode,
        kind: edge.syncMode === 'assincrona' ? 'async' : 'sync',
        ...(edge.link === 'comunicacao' ? { communication: true } : {}),
      }
      source.edges.push(flowEdge)
      remember(lineKey(flowEdge), edge)
    } else {
      source.extraEdges.push(draftOf(edge))
    }
  }
  return source
}

function edgeDraft(
  fromNode: string,
  toNode: string,
  meta: FlowEdgeMeta,
  syncMode: FlowEdgeDraft['syncMode'],
  curve: number,
  link: FlowEdgeDraft['link'] = 'fluxo',
): FlowEdgeDraft {
  return {
    fromNode,
    toNode,
    link,
    syncMode,
    whenEvent: meta.whenEvent,
    isDefault: meta.isDefault,
    action: meta.action,
    kind: meta.requestKind,
    turnLimit: meta.turnLimit,
    curve,
    instructions: meta.instructions,
  }
}

// Todas as ligações do rascunho como o backend as recebe: início, entre agentes, fim e as que o canvas só repassa.
export function flowEdgesOf(draft: FlowDraft): FlowEdgeDraft[] {
  const curveOf = (key: string): number => draft.curves.get(key) ?? NO_CURVE
  const edges: FlowEdgeDraft[] = []
  const leader = draft.completed.value.startAgent
  if (leader !== null) {
    edges.push(edgeDraft(FLOW_START, leader, draft.metas.get(START_LINE_KEY) ?? START_META, 'sincrona', curveOf(START_LINE_KEY)))
  }
  for (const edge of draft.edges.value) {
    const key = lineKey(edge)
    const communication = edge.communication === true
    const meta = draft.metas.get(key) ?? (communication ? COMMUNICATION_META : AGENT_META)
    edges.push(
      edgeDraft(edge.from, edge.to, meta, edge.kind === 'async' ? 'assincrona' : 'sincrona', curveOf(key), communication ? 'comunicacao' : 'fluxo'),
    )
  }
  for (const agent of draft.completed.value.endAgents) {
    const key = endLineKey(agent)
    edges.push(edgeDraft(agent, FLOW_END, draft.metas.get(key) ?? END_META, 'sincrona', curveOf(key)))
  }
  return [...edges, ...draft.extraEdges.value]
}

export function validationInputOf(draft: FlowDraft): ValidationInput {
  return {
    nodes: draft.agents.value.map((agent) => ({ id: agent, y: draft.positions.get(agent)?.y ?? 0 })),
    gateways: draft.gateways.value.map((gateway) => ({ id: gateway.id, kind: gateway.kind })),
    edges: flowEdgesOf(draft).map((edge) => ({
      from: edge.fromNode,
      to: edge.toNode,
      link: edge.link,
      whenEvent: edge.whenEvent,
      isDefault: edge.isDefault,
    })),
  }
}

function pointOf(point: FlowPoint | null): { x: number; y: number } | null {
  return point === null ? null : { x: point.x, y: point.y }
}

// O backend guarda a camada de cada agente; quem ainda não tem posição entra na primeira camada. Sem camada, não salva.
export function toTeamFlowDraft(draft: FlowDraft, snapshot: OrchestrationSnapshot, teamId: string): TeamFlowDraft | null {
  const firstLayer = [...snapshot.layers].sort((a, b) => a.position - b.position)[0]
  const nodes: FlowNodeDraft[] = []
  for (const agent of draft.agents.value) {
    const layerId = snapshot.placements.find((placement) => placement.agentId === agent && placement.teamId === teamId)?.layerId ?? firstLayer?.id
    const point = draft.positions.get(agent)
    if (layerId === undefined || point === undefined) return null
    nodes.push({ agentId: agent, layerId, posX: point.x, posY: point.y })
  }
  return {
    nodes,
    gateways: draft.gateways.value.map((gateway) => {
      const point = draft.positions.get(gateway.id)
      return { id: gateway.id, kind: gateway.kind, posX: point?.x ?? 0, posY: point?.y ?? 0 }
    }),
    edges: flowEdgesOf(draft),
    start: pointOf(draft.endpoints.start),
    end: pointOf(draft.endpoints.end),
  }
}

function sameConfig(config: FlowNodeConfig, placement: OrchestrationSnapshot['placements'][number]): boolean {
  return (
    config.instructions === placement.instructions &&
    config.criteria === placement.criteria &&
    config.instances === placement.maxInstances &&
    config.worktrees === (placement.maxWorktrees ?? 0)
  )
}

// Configurações do agente (instruções, critérios, instâncias, worktrees) não vão no save_team_flow: quem mudou
// é regravado pelo place_agent depois do fluxo salvo.
export function configChanges(draft: FlowDraft, snapshot: OrchestrationSnapshot, teamId: string): OrchPlacementDraft[] {
  const changes: OrchPlacementDraft[] = []
  for (const agent of draft.agents.value) {
    const placement = snapshot.placements.find((candidate) => candidate.agentId === agent && candidate.teamId === teamId)
    const config = draft.configOf(agent)
    if (placement === undefined || sameConfig(config, placement)) continue
    changes.push({
      agentId: agent,
      teamId,
      layerId: placement.layerId,
      instructions: config.instructions,
      maxInstances: config.instances || DEFAULT_INSTANCES,
      criteria: config.criteria,
      maxWorktrees: config.worktrees > 0 ? config.worktrees : null,
    })
  }
  return changes
}

// Traz o que o backend guarda para o rascunho de cada time (também os que ainda não foram abertos), sem
// sobrescrever quem tem mudança não salva; time que sumiu do snapshot perde o rascunho.
export function syncFlowDrafts(snapshot: OrchestrationSnapshot, previousTeamIds: readonly string[]): void {
  const current = new Set(snapshot.teams.map((team) => team.id))
  for (const teamId of previousTeamIds) {
    if (!current.has(teamId)) forgetFlowDraft(teamId)
  }
  for (const team of snapshot.teams) {
    const draft = useFlowDraft(team.id)
    if (!draft.dirty.value) draft.load(flowSourceFrom(snapshot, team.id))
  }
}
