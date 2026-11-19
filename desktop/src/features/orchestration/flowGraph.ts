import type { Agent } from '../team/events'

export interface FlowLink {
  from: Agent
  to: Agent
}

// Síncrona: o emissor espera a resposta. Assíncrona: segue sem esperar.
export type LineKind = 'sync' | 'async'

export interface FlowEdge extends FlowLink {
  kind: LineKind
  // Ligação de comunicação: consulta sem direção entre agentes da mesma linha; não é etapa do fluxo.
  communication?: boolean
}

export interface FlowPoint {
  x: number
  y: number
}

export interface FlowPlacement extends FlowPoint {
  agent: Agent
}

export interface FlowEndpoints {
  start: FlowPoint | null
  end: FlowPoint | null
}

export interface EmptyBox {
  left: number
  top: number
  width: number
  height: number
}

export interface FlowLayout {
  emptyBox: EmptyBox | null
  nodes: FlowPlacement[]
  start: FlowPoint
  end: FlowPoint
}

export interface FlowView {
  zoom: number
  x: number
  y: number
}

export interface CompletedFlow {
  manualEdges: FlowEdge[]
  startAgent: Agent | null
  endAgents: Agent[]
}

export const PORT_SIDES = ['top', 'right', 'bottom', 'left'] as const
export type PortSide = (typeof PORT_SIDES)[number]

// Lado do losango do gateway, em unidades do desenho.
export const GATEWAY_SIZE = 40

export const FLOW_GEOMETRY = {
  nodeWidth: 168,
  nodeHeight: 76,
  rowGap: 72,
  margin: 32,
  endpointRadius: 12,
} as const

// Área fixa do canvas em unidades do desenho: nada sai dela, e o zoom só enquadra a área.
export const CANVAS_AREA = 640
export const ZOOM_MIN = 1
export const ZOOM_MAX = 3
export const ZOOM_STEP = 1.15

const START_Y = FLOW_GEOMETRY.margin + FLOW_GEOMETRY.endpointRadius
const MIN_NODE_Y = START_Y + FLOW_GEOMETRY.rowGap / 2 + FLOW_GEOMETRY.nodeHeight / 2
// O fim fica abaixo do nó mais baixo: o nó não pode descer tanto que o fim saia da área.
const MAX_NODE_Y =
  CANVAS_AREA - FLOW_GEOMETRY.margin - 2 * FLOW_GEOMETRY.endpointRadius - FLOW_GEOMETRY.rowGap - FLOW_GEOMETRY.nodeHeight / 2
const ENDPOINT_LABEL_EXTENT = 28
const EMPTY_BOX_WIDTH = 240
const EMPTY_BOX_HEIGHT = 124
const FRAME_MARGIN = 24
// Sem agentes, o enquadramento não pode ampliar tanto que o início e o fim fiquem gigantes.
const FRAME_MIN_SIZE = 480
const ARC_BEND_RATIO = 0.2
const SNAP_DISTANCE = 6

export function canLink(edges: readonly FlowEdge[], from: Agent, to: Agent): boolean {
  return from !== to && !edges.some((edge) => edge.from === from && edge.to === to)
}

// Início e fim só têm ligação quando o usuário a desenha: sem ela ficam soltos e a validação os marca.
export function completeFlow(
  agents: readonly Agent[],
  manualEdges: readonly FlowEdge[],
  leader: Agent | null,
  endAgents: readonly Agent[],
): CompletedFlow {
  return {
    manualEdges: [...manualEdges],
    startAgent: leader !== null && agents.includes(leader) ? leader : null,
    endAgents: endAgents.filter((agent) => agents.includes(agent)),
  }
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

// Nó fica inteiro dentro da área e abaixo da bolinha de início.
export function clampNode(point: FlowPoint): FlowPoint {
  const halfWidth = FLOW_GEOMETRY.nodeWidth / 2
  const { margin } = FLOW_GEOMETRY
  return {
    x: clamp(point.x, halfWidth + margin, CANVAS_AREA - halfWidth - margin),
    y: clamp(point.y, MIN_NODE_Y, MAX_NODE_Y),
  }
}

// Primeiro lugar livre (varrendo a área da esquerda para a direita, de cima para baixo) que não toca nenhum nó.
export function freeSpot(taken: readonly FlowPoint[]): FlowPoint {
  const stepX = FLOW_GEOMETRY.nodeWidth / 2 + GATEWAY_SIZE
  const stepY = FLOW_GEOMETRY.nodeHeight / 2 + GATEWAY_SIZE
  for (let y = MIN_NODE_Y; y <= MAX_NODE_Y; y += stepY) {
    for (let x = FLOW_GEOMETRY.margin + FLOW_GEOMETRY.nodeWidth / 2; x <= CANVAS_AREA - FLOW_GEOMETRY.margin; x += stepX) {
      const point = clampNode({ x, y })
      const clear = taken.every(
        (other) => Math.abs(other.x - point.x) >= stepX || Math.abs(other.y - point.y) >= stepY,
      )
      if (clear) return point
    }
  }
  return clampNode({ x: CANVAS_AREA / 2, y: MIN_NODE_Y })
}

export function clampEndpoint(point: FlowPoint): FlowPoint {
  const inset = FLOW_GEOMETRY.endpointRadius + FLOW_GEOMETRY.margin
  return {
    x: clamp(point.x, inset, CANVAS_AREA - inset),
    y: clamp(point.y, inset, CANVAS_AREA - inset),
  }
}

export function canvasLayout(
  agents: readonly Agent[],
  positions: ReadonlyMap<Agent, FlowPoint>,
  endpoints: FlowEndpoints,
): FlowLayout {
  const { nodeHeight, rowGap, endpointRadius } = FLOW_GEOMETRY
  const nodes = agents.map((agent) => ({ agent, ...(positions.get(agent) ?? clampNode({ x: CANVAS_AREA / 2, y: MIN_NODE_Y })) }))
  const start = endpoints.start ?? { x: CANVAS_AREA / 2, y: START_Y }
  // Sem agentes, a caixa tracejada de "arraste aqui" fica no lugar do primeiro nó e o fim fica abaixo dela.
  const emptyBox =
    agents.length === 0
      ? { left: start.x - EMPTY_BOX_WIDTH / 2, top: start.y + rowGap, width: EMPTY_BOX_WIDTH, height: EMPTY_BOX_HEIGHT }
      : null
  const lowestBottom = emptyBox
    ? emptyBox.top + emptyBox.height
    : nodes.reduce((bottom, node) => Math.max(bottom, node.y + nodeHeight / 2), START_Y)
  const end = endpoints.end ?? clampEndpoint({ x: CANVAS_AREA / 2, y: lowestBottom + rowGap + endpointRadius })
  return { nodes, start, end, emptyBox }
}

export interface FlowBounds {
  left: number
  top: number
  right: number
  bottom: number
}

// Área ocupada pelo desenho: nós, início e fim (com o ícone do Ember ao lado).
export function contentBounds(layout: FlowLayout): FlowBounds {
  const halfWidth = FLOW_GEOMETRY.nodeWidth / 2
  const halfHeight = FLOW_GEOMETRY.nodeHeight / 2
  const radius = FLOW_GEOMETRY.endpointRadius
  const boxes: FlowBounds[] = [
    ...layout.nodes.map((node) => ({
      left: node.x - halfWidth,
      top: node.y - halfHeight,
      right: node.x + halfWidth,
      bottom: node.y + halfHeight,
    })),
    ...(layout.emptyBox
      ? [
          {
            left: layout.emptyBox.left,
            top: layout.emptyBox.top,
            right: layout.emptyBox.left + layout.emptyBox.width,
            bottom: layout.emptyBox.top + layout.emptyBox.height,
          },
        ]
      : []),
    ...[layout.start, layout.end].map((point) => ({
      left: point.x - radius,
      top: point.y - radius,
      right: point.x + radius + ENDPOINT_LABEL_EXTENT,
      bottom: point.y + radius,
    })),
  ]
  return {
    left: Math.min(...boxes.map((box) => box.left)),
    top: Math.min(...boxes.map((box) => box.top)),
    right: Math.max(...boxes.map((box) => box.right)),
    bottom: Math.max(...boxes.map((box) => box.bottom)),
  }
}

// Visão que enquadra o desenho inteiro na área visível, centralizado. Depende só do conteúdo, não da largura da tela.
// aspect = largura / altura do container: a janela acompanha a proporção da tela, então o canvas ocupa tudo.
export function frameContent(layout: FlowLayout, aspect = 1): FlowView {
  const bounds = contentBounds(layout)
  const contentWidth = bounds.right - bounds.left + 2 * FRAME_MARGIN
  const contentHeight = bounds.bottom - bounds.top + 2 * FRAME_MARGIN
  const height = Math.max(contentHeight, contentWidth / aspect, FRAME_MIN_SIZE)
  const zoom = clamp(CANVAS_AREA / height, ZOOM_MIN, ZOOM_MAX)
  const size = visibleSize(zoom)
  const centerX = (bounds.left + bounds.right) / 2
  const centerY = (bounds.top + bounds.bottom) / 2
  return clampView({ zoom, x: centerX - (size * aspect) / 2, y: centerY - size / 2 }, aspect)
}

// Cabe na janela atual com a margem interna: se não couber, o chamador reenquadra para o início e o fim não ficarem na borda.
export function contentFitsView(layout: FlowLayout, view: FlowView, aspect = 1): boolean {
  const bounds = contentBounds(layout)
  const size = visibleSize(view.zoom)
  return (
    bounds.left >= view.x + FRAME_MARGIN &&
    bounds.right <= view.x + size * aspect - FRAME_MARGIN &&
    bounds.top >= view.y + FRAME_MARGIN &&
    bounds.bottom <= view.y + size - FRAME_MARGIN
  )
}

// Altura visível da janela em unidades do desenho; a largura visível é essa altura vezes a proporção.
export function visibleSize(zoom: number): number {
  return CANVAS_AREA / zoom
}

// Eixo menor que a área: a janela fica dentro dela. Eixo maior (tela mais larga que a área): a área fica centrada.
function clampAxis(value: number, size: number): number {
  if (size >= CANVAS_AREA) return clamp(value, CANVAS_AREA - size, 0)
  return clamp(value, 0, CANVAS_AREA - size)
}

export function clampView(view: FlowView, aspect = 1): FlowView {
  const zoom = clamp(view.zoom, ZOOM_MIN, ZOOM_MAX)
  const height = visibleSize(zoom)
  return {
    zoom,
    x: clampAxis(view.x, height * aspect),
    y: clampAxis(view.y, height),
  }
}

// Zoom com âncora: o ponto sob o cursor continua sob o cursor depois do zoom.
export function zoomAt(view: FlowView, factor: number, anchor: FlowPoint, aspect = 1): FlowView {
  const zoom = clamp(view.zoom * factor, ZOOM_MIN, ZOOM_MAX)
  const oldHeight = visibleSize(view.zoom)
  const newHeight = visibleSize(zoom)
  const fx = (anchor.x - view.x) / (oldHeight * aspect)
  const fy = (anchor.y - view.y) / oldHeight
  return clampView({ zoom, x: anchor.x - fx * newHeight * aspect, y: anchor.y - fy * newHeight }, aspect)
}

export interface HalfSize {
  width: number
  height: number
}

export const AGENT_HALF: HalfSize = { width: FLOW_GEOMETRY.nodeWidth / 2, height: FLOW_GEOMETRY.nodeHeight / 2 }
export const GATEWAY_HALF: HalfSize = { width: GATEWAY_SIZE / 2, height: GATEWAY_SIZE / 2 }

export function portAt(center: FlowPoint, side: PortSide, half: HalfSize = AGENT_HALF): FlowPoint {
  const halfWidth = half.width
  const halfHeight = half.height
  const offsets: Record<PortSide, FlowPoint> = {
    top: { x: 0, y: -halfHeight },
    right: { x: halfWidth, y: 0 },
    bottom: { x: 0, y: halfHeight },
    left: { x: -halfWidth, y: 0 },
  }
  return { x: center.x + offsets[side].x, y: center.y + offsets[side].y }
}

// Ponto na borda do retângulo do nó, na direção de outro ponto: a seta não entra no texto.
export function borderPoint(center: FlowPoint, toward: FlowPoint, half: HalfSize = AGENT_HALF): FlowPoint {
  const halfWidth = half.width
  const halfHeight = half.height
  const dx = toward.x - center.x
  const dy = toward.y - center.y
  const scaleX = dx === 0 ? Infinity : halfWidth / Math.abs(dx)
  const scaleY = dy === 0 ? Infinity : halfHeight / Math.abs(dy)
  const scale = Math.min(scaleX, scaleY)
  return { x: center.x + dx * scale, y: center.y + dy * scale }
}

// Seta em arco: curva quadrática; o deslocamento (com sinal) é o que o usuário muda arrastando a bolinha do meio.
// Sem deslocamento salvo, a curva se abre para um lado fixo, então A→B e B→A não se sobrepõem.
export function arcPath(from: FlowPoint, to: FlowPoint, offset?: number): { d: string; middle: FlowPoint } {
  const dx = to.x - from.x
  const dy = to.y - from.y
  const length = Math.hypot(dx, dy) || 1
  const bend = offset ?? length * ARC_BEND_RATIO
  const control = {
    x: (from.x + to.x) / 2 - (dy / length) * bend,
    y: (from.y + to.y) / 2 + (dx / length) * bend,
  }
  return {
    d: `M ${from.x} ${from.y} Q ${control.x} ${control.y} ${to.x} ${to.y}`,
    middle: {
      x: 0.25 * from.x + 0.5 * control.x + 0.25 * to.x,
      y: 0.25 * from.y + 0.5 * control.y + 0.25 * to.y,
    },
  }
}

// Deslocamento que faz a curva passar pela bolinha do meio (o ponto médio do arco fica na metade do controle).
export function bendFromHandle(from: FlowPoint, to: FlowPoint, handle: FlowPoint): number {
  const dx = to.x - from.x
  const dy = to.y - from.y
  const length = Math.hypot(dx, dy) || 1
  const normalX = -dy / length
  const normalY = dx / length
  const chordX = (from.x + to.x) / 2
  const chordY = (from.y + to.y) / 2
  return 2 * ((handle.x - chordX) * normalX + (handle.y - chordY) * normalY)
}

export function lineKey(link: FlowLink): string {
  return `${link.from}>${link.to}`
}

// Chaves das linhas do início (uma só) e do fim (uma por agente ligado a ele).
export const START_LINE_KEY = 'inicio'

export function endLineKey(agent: Agent): string {
  return `fim:${agent}`
}

// Encaixe ao arrastar com Shift: centro igual ao de outro nó, na vertical ou na horizontal.
export function snapPoint(point: FlowPoint, others: readonly FlowPoint[]): FlowPoint {
  const near = (value: number, axis: 'x' | 'y') => {
    const match = others.find((other) => Math.abs(other[axis] - value) <= SNAP_DISTANCE)
    return match ? match[axis] : value
  }
  return { x: near(point.x, 'x'), y: near(point.y, 'y') }
}

export function agentAt(nodes: readonly FlowPlacement[], point: FlowPoint): Agent | null {
  const halfWidth = FLOW_GEOMETRY.nodeWidth / 2
  const halfHeight = FLOW_GEOMETRY.nodeHeight / 2
  const hit = nodes.find(
    (node) => Math.abs(point.x - node.x) <= halfWidth && Math.abs(point.y - node.y) <= halfHeight,
  )
  return hit?.agent ?? null
}

export function agentsInside(nodes: readonly FlowPlacement[], from: FlowPoint, to: FlowPoint): Agent[] {
  const left = Math.min(from.x, to.x)
  const right = Math.max(from.x, to.x)
  const top = Math.min(from.y, to.y)
  const bottom = Math.max(from.y, to.y)
  return nodes
    .filter((node) => node.x >= left && node.x <= right && node.y >= top && node.y <= bottom)
    .map((node) => node.agent)
}
