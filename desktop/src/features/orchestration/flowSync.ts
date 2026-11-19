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
  forgetFlowDraft,
  useFlowDraft,
  type FlowDraft,
  type FlowEdgeMeta,
  type FlowNodeConfig,
  type FlowSource,
} from './useFlowDraft'

const START_META: FlowEdgeMeta = { whenEvent: 'plano_aprovado', action: 'delegar', requestKind: 'planejar', turnLimit: null }
const AGENT_META: FlowEdgeMeta = { whenEvent: 'tarefa_concluida', action: 'delegar', requestKind: 'implementar', turnLimit: null }
const END_META: FlowEdgeMeta = { whenEvent: 'tarefa_concluida', action: 'escalar', requestKind: 'revisar', turnLimit: null }
// Sem curva gravada, a linha usa a curva padrão do canvas.
const NO_CURVE = 0

function metaOf(edge: OrchFlowEdge): FlowEdgeMeta {
  return { whenEvent: edge.whenEvent, action: edge.action, requestKind: edge.kind, turnLimit: edge.turnLimit }
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
  }
}

// O fluxo de um time como o backend o guarda: agentes do time, ligações entre eles, início, fim e configurações.
export function flowSourceFrom(snapshot: OrchestrationSnapshot, teamId: string): FlowSource {
  const team = snapshot.teams.find((candidate) => candidate.id === teamId)
  const placements = snapshot.placements.filter((placement) => placement.teamId === teamId)
  const agents = placements.map((placement) => placement.agentId)
  const known = new Set<string>(agents)
  const source: FlowSource = {
    agents,
    positions: new Map(placements.map((placement) => [placement.agentId, clampNode({ x: placement.posX, y: placement.posY })])),
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
    extraGateways: snapshot.gateways
      .filter((gateway) => gateway.teamId === teamId)
      .map((gateway) => ({ id: gateway.id, kind: gateway.kind, posX: gateway.posX, posY: gateway.posY })),
    extraEdges: [],
  }
  const remember = (key: string, edge: OrchFlowEdge): void => {
    source.metas.set(key, metaOf(edge))
    if (edge.curve !== NO_CURVE) source.curves.set(key, edge.curve)
  }
  for (const edge of snapshot.edges.filter((candidate) => candidate.teamId === teamId)) {
    const fromAgent = known.has(edge.fromNode)
    const toAgent = known.has(edge.toNode)
    if (edge.link === 'fluxo' && edge.fromNode === FLOW_START && toAgent && source.leader === null) {
      source.leader = edge.toNode
      remember(START_LINE_KEY, edge)
    } else if (edge.link === 'fluxo' && edge.toNode === FLOW_END && fromAgent) {
      source.endAgents.push(edge.fromNode)
      remember(endLineKey(edge.fromNode), edge)
    } else if (edge.link === 'fluxo' && fromAgent && toAgent) {
      const flowEdge: FlowEdge = { from: edge.fromNode, to: edge.toNode, kind: edge.syncMode === 'assincrona' ? 'async' : 'sync' }
      source.edges.push(flowEdge)
      remember(lineKey(flowEdge), edge)
    } else {
      source.extraEdges.push(draftOf(edge))
    }
  }
  return source
}

function edgeDraft(fromNode: string, toNode: string, meta: FlowEdgeMeta, syncMode: FlowEdgeDraft['syncMode'], curve: number): FlowEdgeDraft {
  return {
    fromNode,
    toNode,
    link: 'fluxo',
    syncMode,
    whenEvent: meta.whenEvent,
    isDefault: false,
    action: meta.action,
    kind: meta.requestKind,
    turnLimit: meta.turnLimit,
    curve,
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
    edges.push(edgeDraft(edge.from, edge.to, draft.metas.get(key) ?? AGENT_META, edge.kind === 'async' ? 'assincrona' : 'sincrona', curveOf(key)))
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
    gateways: draft.extraGateways.value.map((gateway) => ({ id: gateway.id, kind: gateway.kind })),
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
    gateways: [...draft.extraGateways.value],
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
