import { computed, reactive, ref } from 'vue'
import type { Agent } from '../team/events'
import {
  canLink,
  clampEndpoint,
  clampNode,
  completeFlow,
  endLineKey,
  FLOW_GEOMETRY,
  START_LINE_KEY,
  type FlowEdge,
  type FlowEndpoints,
  type FlowPoint,
  type LineKind,
} from './flowGraph'
import { DEFAULT_INSTANCES, DEFAULT_WORKTREES } from './orchestrationForm'
import type {
  EdgeAction,
  FlowEdgeDraft,
  FlowGatewayDraft,
  RequestKind,
  WhenEvent,
} from './orchestrationTypes'

export interface FlowNodeConfig {
  instructions: string
  criteria: string
  instances: number
  worktrees: number
}

// O que a ligação guarda no backend além do desenho: evento, ação, tipo do pedido e limite de turnos da linha.
export interface FlowEdgeMeta {
  whenEvent: WhenEvent
  action: EdgeAction
  requestKind: RequestKind
  turnLimit: number | null
}

// Estado do fluxo como o backend o guarda; o que o canvas ainda não desenha (gateways e ligações de comunicação)
// passa intacto e é regravado no salvar.
export interface FlowSource {
  agents: Agent[]
  positions: Map<Agent, FlowPoint>
  edges: FlowEdge[]
  leader: Agent | null
  endAgents: Agent[]
  configs: Map<Agent, FlowNodeConfig>
  endpoints: FlowEndpoints
  curves: Map<string, number>
  metas: Map<string, FlowEdgeMeta>
  extraGateways: FlowGatewayDraft[]
  extraEdges: FlowEdgeDraft[]
}

export type AlignAxis = 'vertical' | 'horizontal'
// Centro alinha o eixo do meio; borda alinha a aresta esquerda (vertical) ou superior (horizontal).
export type AlignMode = 'center' | 'border'

const DEFAULT_NODE_CONFIG: FlowNodeConfig = {
  instructions: '',
  criteria: '',
  instances: DEFAULT_INSTANCES,
  worktrees: DEFAULT_WORKTREES,
}

// Rascunho do fluxo de um time: nada é gravado até o Salvar fluxo; `load` traz o que o backend guarda.
function createFlowDraft() {
  const agents = ref<Agent[]>([])
  const edges = ref<FlowEdge[]>([])
  const positions = reactive(new Map<Agent, FlowPoint>())
  const endpoints = reactive<FlowEndpoints>({ start: null, end: null })
  const leaderChoice = ref<Agent | null>(null)
  const configs = reactive(new Map<Agent, FlowNodeConfig>())
  const curves = reactive(new Map<string, number>())
  const endAgents = ref<Agent[]>([])
  const metas = reactive(new Map<string, FlowEdgeMeta>())
  const extraGateways = ref<FlowGatewayDraft[]>([])
  const extraEdges = ref<FlowEdgeDraft[]>([])
  const loaded = ref(false)
  const baseline = ref('')

  const leader = computed(() =>
    leaderChoice.value !== null && agents.value.includes(leaderChoice.value) ? leaderChoice.value : null,
  )
  const completed = computed(() => completeFlow(agents.value, edges.value, leader.value, endAgents.value))

  // Texto estável do estado inteiro: o rascunho está sujo quando difere do que foi carregado ou salvo.
  function serialize(): string {
    const entries = <T>(map: ReadonlyMap<string, T>) => [...map.entries()].sort(([a], [b]) => a.localeCompare(b))
    return JSON.stringify({
      agents: agents.value,
      edges: edges.value,
      leader: leader.value,
      endAgents: endAgents.value,
      positions: entries(positions),
      configs: entries(configs),
      endpoints: { start: endpoints.start, end: endpoints.end },
      curves: entries(curves),
      metas: entries(metas),
      extraGateways: extraGateways.value,
      extraEdges: extraEdges.value,
    })
  }
  const dirty = computed(() => loaded.value && serialize() !== baseline.value)

  function load(source: FlowSource): void {
    agents.value = [...source.agents]
    edges.value = [...source.edges]
    leaderChoice.value = source.leader
    endAgents.value = [...source.endAgents]
    endpoints.start = source.endpoints.start
    endpoints.end = source.endpoints.end
    for (const [target, from] of [
      [positions, source.positions],
      [configs, source.configs],
      [curves, source.curves],
      [metas, source.metas],
    ] as const) {
      ;(target as Map<string, unknown>).clear()
      for (const [key, value] of from as Map<string, unknown>) (target as Map<string, unknown>).set(key, value)
    }
    extraGateways.value = [...source.extraGateways]
    extraEdges.value = [...source.extraEdges]
    loaded.value = true
    baseline.value = serialize()
  }

  function markSaved(): void {
    baseline.value = serialize()
  }

  // Cada elemento fica onde foi solto: adicionar ou mover um agente não reorganiza os outros.
  function addAgent(agent: Agent, point: FlowPoint): void {
    if (agents.value.includes(agent)) return
    // O primeiro agente do fluxo vira o líder: a ligação do início nasce com ele.
    if (agents.value.length === 0 && leaderChoice.value === null) leaderChoice.value = agent
    agents.value = [...agents.value, agent]
    positions.set(agent, clampNode(point))
  }

  function moveAgents(group: readonly Agent[], dx: number, dy: number): void {
    for (const agent of group) {
      const point = positions.get(agent)
      if (point) positions.set(agent, clampNode({ x: point.x + dx, y: point.y + dy }))
    }
  }

  function moveEndpoint(which: 'start' | 'end', point: FlowPoint): void {
    endpoints[which] = clampEndpoint(point)
  }

  // Toda linha nova nasce síncrona; o usuário troca o tipo na própria linha selecionada.
  function link(from: Agent, to: Agent, kind: LineKind = 'sync'): void {
    if (!canLink(edges.value, from, to)) return
    edges.value = [...edges.value, { from, to, kind }]
  }

  function setKind(from: Agent, to: Agent, kind: LineKind): void {
    edges.value = edges.value.map((edge) => (edge.from === from && edge.to === to ? { ...edge, kind } : edge))
  }

  // Tira o agente do fluxo com as setas ligadas a ele; as ligações automáticas são recalculadas sozinhas.
  function removeAgent(agent: Agent): void {
    if (!agents.value.includes(agent)) return
    agents.value = agents.value.filter((member) => member !== agent)
    edges.value = edges.value.filter((edge) => edge.from !== agent && edge.to !== agent)
    endAgents.value = endAgents.value.filter((member) => member !== agent)
    positions.delete(agent)
    configs.delete(agent)
    if (leaderChoice.value === agent) leaderChoice.value = null
    for (const key of [...curves.keys()]) {
      if (key.includes(agent)) curves.delete(key)
    }
    for (const key of [...metas.keys()]) {
      if (key.includes(agent)) metas.delete(key)
    }
  }

  function unlink(from: Agent, to: Agent): void {
    edges.value = edges.value.filter((edge) => edge.from !== from || edge.to !== to)
  }

  // Início e fim: ligar põe a linha; remover a linha deixa o ponto solto (e a validação o marca).
  function linkStart(agent: Agent): void {
    if (agents.value.includes(agent)) leaderChoice.value = agent
  }

  function unlinkStart(): void {
    leaderChoice.value = null
    metas.delete(START_LINE_KEY)
  }

  function linkEnd(agent: Agent): void {
    if (agents.value.includes(agent) && !endAgents.value.includes(agent)) endAgents.value = [...endAgents.value, agent]
  }

  function unlinkEnd(agent: Agent): void {
    endAgents.value = endAgents.value.filter((member) => member !== agent)
    metas.delete(endLineKey(agent))
  }

  // Curvatura salva por linha (chave da linha); ausente = curva padrão.
  function setCurve(key: string, offset: number): void {
    curves.set(key, offset)
  }

  // Máximo de um líder: o novo líder troca de lugar com o atual; as conexões de ambos continuam.
  function setLeader(agent: Agent): void {
    if (!agents.value.includes(agent)) return
    const previous = leader.value
    if (previous !== null && previous !== agent) swapPositions(previous, agent)
    leaderChoice.value = agent
  }

  function swapPositions(first: Agent, second: Agent): void {
    const firstPoint = positions.get(first)
    const secondPoint = positions.get(second)
    if (!firstPoint || !secondPoint) return
    positions.set(first, secondPoint)
    positions.set(second, firstPoint)
  }

  // Alinha o grupo pela média: vertical deixa todos na mesma coluna (x), horizontal na mesma linha (y).
  function alignAgents(mode: AlignMode, axis: AlignAxis, group: readonly Agent[]): void {
    const points = group.map((agent) => positions.get(agent)).filter((point): point is FlowPoint => point !== undefined)
    if (points.length < 2) return
    const key = axis === 'vertical' ? 'x' : 'y'
    const half = axis === 'vertical' ? FLOW_GEOMETRY.nodeWidth / 2 : FLOW_GEOMETRY.nodeHeight / 2
    const offset = mode === 'border' ? half : 0
    const mean = points.reduce((sum, point) => sum + point[key] - offset, 0) / points.length + offset
    for (const agent of group) {
      const point = positions.get(agent)
      if (point) positions.set(agent, clampNode({ ...point, [key]: mean }))
    }
  }

  function configOf(agent: Agent): FlowNodeConfig {
    return configs.get(agent) ?? DEFAULT_NODE_CONFIG
  }

  function setConfig(agent: Agent, patch: Partial<FlowNodeConfig>): void {
    configs.set(agent, { ...configOf(agent), ...patch })
  }

  return {
    agents,
    edges,
    positions,
    endpoints,
    leader,
    completed,
    addAgent,
    moveAgents,
    moveEndpoint,
    curves,
    metas,
    endAgents,
    extraGateways,
    extraEdges,
    dirty,
    loaded,
    load,
    markSaved,
    link,
    setKind,
    unlink,
    linkStart,
    unlinkStart,
    linkEnd,
    unlinkEnd,
    removeAgent,
    setCurve,
    setLeader,
    alignAgents,
    configOf,
    setConfig,
  }
}

export type FlowDraft = ReturnType<typeof createFlowDraft>

// Cada time tem o seu fluxo: o desenho fica guardado por time enquanto a tela estiver aberta.
const flowDrafts = new Map<string, FlowDraft>()

export function useFlowDraft(teamId: string): FlowDraft {
  const existing = flowDrafts.get(teamId)
  if (existing) return existing
  const draft = createFlowDraft()
  flowDrafts.set(teamId, draft)
  return draft
}

// Leitura sem criar: a lista de times mostra o fluxo só de quem já foi aberto nesta sessão.
export function peekFlowDraft(teamId: string): FlowDraft | undefined {
  return flowDrafts.get(teamId)
}

export function forgetFlowDraft(teamId: string): void {
  flowDrafts.delete(teamId)
}
