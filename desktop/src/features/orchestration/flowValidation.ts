import {
  FLOW_END,
  FLOW_START,
  type FlowLink,
  type FlowProblem,
  type GatewayKind,
  type WhenEvent,
} from './orchestrationTypes'

// Metade da altura do nó: dois agentes mais próximos que isso na vertical estão na mesma linha (igual ao backend).
export const SAME_LEVEL_TOLERANCE = 38
const GATEWAY_MIN_BRANCHES = 2

export interface ValidationNode {
  id: string
  y: number
}

export interface ValidationGateway {
  id: string
  kind: GatewayKind
}

export interface ValidationEdge {
  from: string
  to: string
  link: FlowLink
  whenEvent: WhenEvent
  isDefault: boolean
}

export interface ValidationInput {
  nodes: readonly ValidationNode[]
  gateways: readonly ValidationGateway[]
  edges: readonly ValidationEdge[]
}

const START_NEEDS_ONE_EXIT = 'O início precisa de exatamente uma ligação de saída.'
const END_NEEDS_ENTRY = 'O fim precisa de ao menos uma ligação de entrada.'
const COMMUNICATION_SAME_ROW = 'A comunicação liga só agentes da mesma linha.'

function problem(code: FlowProblem['code'], node: string | null, message: string): FlowProblem {
  return { code, node, message }
}

function reach(edges: readonly ValidationEdge[], from: string, backwards: boolean): Set<string> {
  const seen = new Set<string>()
  const pending = [from]
  while (pending.length > 0) {
    const node = pending.pop() as string
    for (const edge of edges) {
      const near = backwards ? edge.to : edge.from
      const far = backwards ? edge.from : edge.to
      if (near === node && !seen.has(far)) {
        seen.add(far)
        pending.push(far)
      }
    }
  }
  return seen
}

function gatewayIsValid(flow: readonly ValidationEdge[], gateway: ValidationGateway): boolean {
  const entries = flow.filter((edge) => edge.to === gateway.id)
  const exits = flow.filter((edge) => edge.from === gateway.id)
  if (gateway.kind === 'paralelo') {
    return (
      (entries.length === 1 && exits.length >= GATEWAY_MIN_BRANCHES) ||
      (entries.length >= GATEWAY_MIN_BRANCHES && exits.length === 1)
    )
  }
  const events = new Set(exits.map((edge) => edge.whenEvent))
  return (
    entries.length === 1 &&
    exits.length >= GATEWAY_MIN_BRANCHES &&
    events.size === exits.length &&
    exits.filter((edge) => edge.isDefault).length === 1
  )
}

// Mesmas regras V1 a V7 do backend (orchestration/flow_validation.rs), na mesma ordem; fluxo vazio é válido.
// `nameOf` dá o nome de exibição de um agente.
export function validateFlow(input: ValidationInput, nameOf: (id: string) => string): FlowProblem[] {
  const { nodes, gateways, edges } = input
  if (nodes.length === 0 && gateways.length === 0 && edges.length === 0) return []
  const flow = edges.filter((edge) => edge.link === 'fluxo')
  const gatewayKind = new Map(gateways.map((gateway) => [gateway.id, gateway.kind]))
  const levelOf = new Map(nodes.map((node) => [node.id, node.y]))
  const label = (id: string): string => {
    const kind = gatewayKind.get(id)
    return kind === undefined ? nameOf(id) : `gateway ${kind}`
  }
  const problems: FlowProblem[] = []

  if (flow.filter((edge) => edge.from === FLOW_START).length !== 1) {
    problems.push(problem('V1', FLOW_START, START_NEEDS_ONE_EXIT))
  }
  if (!flow.some((edge) => edge.to === FLOW_END)) problems.push(problem('V2', FLOW_END, END_NEEDS_ENTRY))
  for (const node of nodes) {
    if (!flow.some((edge) => edge.to === node.id) || !flow.some((edge) => edge.from === node.id)) {
      problems.push(problem('V3', node.id, `${label(node.id)} precisa de uma ligação de entrada e uma de saída.`))
    }
  }
  const fromStart = reach(flow, FLOW_START, false)
  const toEnd = reach(flow, FLOW_END, true)
  for (const id of [...nodes.map((node) => node.id), ...gateways.map((gateway) => gateway.id)]) {
    if (!fromStart.has(id) || !toEnd.has(id)) {
      problems.push(problem('V4', id, `${label(id)} não está no caminho do início ao fim.`))
    }
  }
  for (const gateway of gateways) {
    if (!gatewayIsValid(flow, gateway)) {
      problems.push(problem('V5', gateway.id, `O gateway ${gateway.kind} está com ligações inválidas.`))
    }
  }
  for (const edge of edges.filter((candidate) => candidate.link === 'comunicacao')) {
    const from = levelOf.get(edge.from)
    const to = levelOf.get(edge.to)
    const sameRow = from !== undefined && to !== undefined && Math.abs(from - to) <= SAME_LEVEL_TOLERANCE
    if (!sameRow) problems.push(problem('V6', edge.from, COMMUNICATION_SAME_ROW))
  }
  const known = (id: string): boolean =>
    id === FLOW_START || id === FLOW_END || levelOf.has(id) || gatewayKind.has(id)
  const reported = new Set<string>()
  for (const edge of edges) {
    for (const id of [edge.from, edge.to]) {
      if (!known(id) && !reported.has(id)) {
        reported.add(id)
        problems.push(problem('V7', id, `${nameOf(id)} não é deste time.`))
      }
    }
  }
  return problems
}

export function problemCountLabel(count: number): string {
  return count === 1 ? '1 problema' : `${count} problemas`
}
