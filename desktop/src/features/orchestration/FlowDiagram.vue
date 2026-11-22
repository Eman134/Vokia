<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import {
  DEFAULT_JITTER,
  sketchDiamond,
  sketchQuadratic,
  sketchRect,
  sketchScribble,
} from '../../ui/sketch/sketchPath'
import AgentPortrait from '../team/AgentPortrait.vue'
import { DEFAULT_RECIPIENT } from '../team/agents'
import type { Agent } from '../team/events'
import { useRoster } from '../team/useRoster'
import { VIEW_COPY } from './orchestrationLabels'
import { DOT_GRID_CLASSES } from '../../ui/dotGrid'
import { FLOW_END, FLOW_START } from './orchestrationTypes'
import {
  AGENT_HALF,
  CANVAS_AREA,
  FLOW_GEOMETRY,
  GATEWAY_HALF,
  PORT_SIDES,
  ZOOM_MIN,
  ZOOM_STEP,
  agentAt,
  agentsInside,
  arcPath,
  bendFromHandle,
  borderPoint,
  canvasLayout,
  clampView,
  contentFitsView,
  endLineKey,
  frameContent,
  hashSeed,
  lineKey,
  portAt,
  snapPoint,
  START_LINE_KEY,
  visibleSize,
  zoomAt,
  type CompletedFlow,
  type FlowEdge,
  type FlowEndpoints,
  type FlowPoint,
  type FlowView,
  type HalfSize,
  type LineKind,
  type PortSide,
} from './flowGraph'
import type { DraftGateway, FlowNodeConfig } from './useFlowDraft'

const ARROWHEAD_ID = 'flow-arrowhead'
const ARROWHEAD_INK_ID = 'flow-arrowhead-ink'
const SHADOW_OFFSET = 3
const NODE_ERROR_SCRIBBLE_SIZE = { width: 22, height: 10 }
const NODE_ERROR_SCRIBBLE_INSET = 28
const NODE_ERROR_SCRIBBLE_TOP = 6
const NODE_SELECTED_WIDTH = 2
const NODE_STROKE_WIDTH = 1.4
const ARROW_WIDTH = 1.5
const ARROW_SELECTED_WIDTH = 2.5
const ARROW_HIT_WIDTH = 14
const ASYNC_DASH = '6 4'
const ENDPOINT_ARROW_WIDTH = 2
const COMMUNICATION_DASH = '2 3'
const COMMUNICATION_LABEL_DY = -6
const GATEWAY_ICON_ARM = 7
const LEGEND_SAMPLE_WIDTH = 28
const LEGEND_SAMPLE_Y = 4
const PREVIEW_DASH = '6 5'
const HANDLE_RADIUS = 6
const TOOLBAR_HEIGHT = 24
const TOOLBAR_GAP = 6
const TOOLBAR_OFFSET = 22
const TOOLBAR_WIDTHS = { sync: 76, async: 86, remove: 84 } as const
const PORT_RADIUS = 5
const PORT_STROKE_WIDTH = 1.5
const NODE_RADIUS = 14
const DRAG_THRESHOLD_PX = 4
const PORTRAIT_SIZE = 28
const PORTRAIT_INSET = 10
const TEXT_X_OFFSET = 46
const NAME_DY = 26
const ROLE_DY = 41
const SUMMARY_DY = 62
const SUMMARY_ICON_SIZE = 14
const WORKTREES_OFFSET = 90
const LEADER_PILL_WIDTH = 48
const LEADER_PILL_HEIGHT = 18
const LEADER_PILL_RADIUS = 9
const LEADER_PILL_OFFSET = 9
const END_INNER_RADIUS = 5
const EMBER_ICON_SIZE = 20
const EMBER_ICON_GAP = 2
const ENDPOINT_LABEL_GAP = 6
// Estado vazio: cartão-guia abaixo do início, onde o primeiro agente vai entrar.
const EMPTY_ICON_RADIUS = 12
const EMPTY_ICON_ARM = 6
const EMPTY_ICON_DY = 30
const EMPTY_TITLE_DY = 62
const EMPTY_STEP_DYS = [86, 104] as const
const EMPTY_STEPS = [VIEW_COPY.emptyFlowStepOne, VIEW_COPY.emptyFlowStepTwo] as const
const LEGEND_SAMPLES = [
  { label: VIEW_COPY.lineSync, dash: undefined, ink: false },
  { label: VIEW_COPY.lineAsync, dash: ASYNC_DASH, ink: false },
  { label: VIEW_COPY.lineAuto, dash: undefined, ink: true },
  { label: VIEW_COPY.lineCommunication, dash: COMMUNICATION_DASH, ink: false },
] as const
const MARQUEE_DASH = '4 3'
const MARQUEE_OPACITY = 0.12
const INSTANCES_ICON = 'M1.5 4h7v7h-7zM4.5 1.5h8v8'
const WORKTREES_ICON = 'M3.5 1.5v11M10.5 1.5v3.5c0 2-7 1.5-7 4.5'
const PAN_BUTTON = 1
const PRIMARY_BUTTON = 0
const TWO_FINGERS = 2

const props = defineProps<{
  agents: Agent[]
  gateways: readonly DraftGateway[]
  positions: ReadonlyMap<Agent, FlowPoint>
  endpoints: FlowEndpoints
  links: CompletedFlow
  curves: ReadonlyMap<string, number>
  selectedAgent: Agent | null
  groupSelected: readonly Agent[]
  selectedLine: string | null
  configOf: (agent: Agent) => FlowNodeConfig
  // Início, fim, agentes e gateways com problema de validação: ficam no token de erro e mostram a mensagem ao passar o mouse.
  problems: ReadonlyMap<string, readonly string[]>
}>()
const emit = defineEmits<{
  select: [agent: Agent, additive: boolean]
  moveNodes: [agents: Agent[], dx: number, dy: number]
  moveEndpoint: [which: 'start' | 'end', point: FlowPoint]
  marquee: [agents: Agent[]]
  clearSelection: []
  link: [from: Agent, to: Agent]
  // As alturas são as do desenho (posição automática inclusa), não só as gravadas.
  linkCommunication: [from: Agent, to: Agent, fromY: number, toY: number]
  selectLine: [key: string]
  selectEndpoint: [which: 'start' | 'end']
  bend: [key: string, offset: number]
  setKind: [edge: FlowEdge, kind: LineKind]
  unlink: [edge: FlowEdge]
  removeLine: [key: string]
  linkStart: [to: Agent]
  linkEnd: [from: Agent]
}>()

interface LinkDrag {
  from: string
  communication: boolean
  start: FlowPoint
  pointer: FlowPoint
}
interface Line {
  key: string
  d: string
  sketch: string
  middle: FlowPoint
  start: FlowPoint
  end: FlowPoint
  edge: FlowEdge | null
  dash: string | undefined
  arrow: boolean
  label: string | null
  // Linha do início ou do fim: traço próprio (cheio, na cor do texto), diferente das ligações entre nós.
  endpoint: boolean
}
type PointerAction =
  | { kind: 'node'; agent: Agent; offset: FlowPoint; moved: boolean; additive: boolean; clientX: number; clientY: number }
  | { kind: 'endpoint'; which: 'start' | 'end'; offset: FlowPoint; moved: boolean; clientX: number; clientY: number }
  | { kind: 'curve'; key: string; start: FlowPoint; end: FlowPoint }
  | { kind: 'marquee'; start: FlowPoint; additive: boolean; moved: boolean; clientX: number; clientY: number }
  | {
      kind: 'pan'
      touch: boolean
      moved: boolean
      startX: number
      startY: number
      lastX: number
      lastY: number
    }

const roster = useRoster()
const svgRef = ref<SVGSVGElement | null>(null)
const containerRef = ref<HTMLDivElement | null>(null)
const view = ref<FlowView>({ zoom: ZOOM_MIN, x: 0, y: 0 })
const linkDrag = ref<LinkDrag | null>(null)
const marqueeRect = ref<{ from: FlowPoint; to: FlowPoint } | null>(null)
const hoveredAgent = ref<Agent | null>(null)
// Ao sair do nó o ponteiro cruza a borda antes de chegar à bolinha: a saída espera um pouco para não desmontá-la.
const HOVER_LINGER_MS = 300
let hoverTimer: ReturnType<typeof setTimeout> | null = null

function enterAgent(agent: Agent): void {
  cancelHoverClear()
  hoveredAgent.value = agent
}

function leaveAgent(agent: Agent): void {
  cancelHoverClear()
  hoverTimer = setTimeout(() => {
    hoverTimer = null
    if (hoveredAgent.value === agent) hoveredAgent.value = null
  }, HOVER_LINGER_MS)
}

function cancelHoverClear(): void {
  if (hoverTimer === null) return
  clearTimeout(hoverTimer)
  hoverTimer = null
}
const hoveredEndpoint = ref<'start' | 'end' | null>(null)
// Fora do ref: o arraste muda a posição a cada movimento e não precisa de reatividade própria.
let action: PointerAction | null = null
let spacePressed = false
const touchPoints = new Map<number, FlowPoint>()
// Enquadramento só acompanha a largura do container e só enquanto o usuário não mexeu na visão.
let zoomTouched = false
let lastWidth = 0

// Proporção do container: a janela de visão tem a mesma proporção, então o desenho cobre a tela inteira.
const aspect = ref(1)

// Mudança de altura (ex.: a barra de Organizar aparecendo) não reenquadra; adicionar ou mover nós também não.
const resizeObserver = new ResizeObserver(([entry]) => {
  if (!entry) return
  const { width, height } = entry.contentRect
  aspect.value = height > 0 ? width / height : 1
  if (zoomTouched || width === lastWidth) return
  lastWidth = width
  view.value = frameContent(layout.value, aspect.value)
})

const viewSize = computed(() => visibleSize(view.value.zoom))
const viewWidth = computed(() => viewSize.value * aspect.value)
const viewBox = computed(() => `${view.value.x} ${view.value.y} ${viewWidth.value} ${viewSize.value}`)
const layout = computed(() => canvasLayout(props.agents, props.positions, props.endpoints))
const emptyBox = computed(() => layout.value.emptyBox)
const placementOf = computed(() => new Map(layout.value.nodes.map((node) => [node.agent, node])))
const gatewayNodes = computed(() =>
  props.gateways.map((gateway) => ({ ...gateway, ...(props.positions.get(gateway.id) ?? { x: CANVAS_AREA / 2, y: CANVAS_AREA / 2 }) })),
)
const gatewayById = computed(() => new Map(gatewayNodes.value.map((gateway) => [gateway.id, gateway])))

// O traço de um nó só depende do id: guardado, arrastar o nó apenas move o path pronto.
const nodeSketches = new Map<string, string>()

function nodeSketch(id: string): string {
  let path = nodeSketches.get(id)
  if (path === undefined) {
    path = sketchRect(FLOW_GEOMETRY.nodeWidth, FLOW_GEOMETRY.nodeHeight, hashSeed(id), DEFAULT_JITTER)
    nodeSketches.set(id, path)
  }
  return path
}

function gatewaySketch(id: string): string {
  let path = nodeSketches.get(id)
  if (path === undefined) {
    path = sketchDiamond(GATEWAY_HALF.width * 2, GATEWAY_HALF.height * 2, hashSeed(id), DEFAULT_JITTER)
    nodeSketches.set(id, path)
  }
  return path
}

const NODE_ERROR_SCRIBBLE = sketchScribble(
  NODE_ERROR_SCRIBBLE_SIZE.width,
  NODE_ERROR_SCRIBBLE_SIZE.height,
  hashSeed('node-error'),
  DEFAULT_JITTER,
)

// Centro e meia medida de qualquer nó do desenho: agente (cartão) ou gateway (losango).
function nodeCenter(id: string): FlowPoint | undefined {
  return placementOf.value.get(id) ?? gatewayById.value.get(id)
}

function halfOf(id: string): HalfSize {
  return gatewayById.value.has(id) ? GATEWAY_HALF : AGENT_HALF
}

function nodeAt(point: FlowPoint): string | null {
  const agent = agentAt(layout.value.nodes, point)
  if (agent !== null) return agent
  const hit = gatewayNodes.value.find(
    (gateway) => Math.abs(point.x - gateway.x) <= GATEWAY_HALF.width && Math.abs(point.y - gateway.y) <= GATEWAY_HALF.height,
  )
  return hit?.id ?? null
}
const linkPreviewPath = computed(() => (linkDrag.value ? linePath(linkDrag.value.start, linkDrag.value.pointer) : ''))
const marqueeBox = computed(() => {
  const rect = marqueeRect.value
  if (!rect) return null
  return {
    x: Math.min(rect.from.x, rect.to.x),
    y: Math.min(rect.from.y, rect.to.y),
    width: Math.abs(rect.to.x - rect.from.x),
    height: Math.abs(rect.to.y - rect.from.y),
  }
})

// Todas as linhas do desenho: entre agentes (tipo e remoção) e as do início e do fim (só remoção).
const lines = computed<Line[]>(() => {
  const result: Line[] = []
  const add = (
    key: string,
    from: FlowPoint,
    to: FlowPoint,
    edge: FlowEdge | null,
    dash: string | undefined,
    communication = false,
    endpoint = false,
  ) => {
    const arc = arcPath(from, to, props.curves.get(key))
    result.push({
      key,
      d: arc.d,
      sketch: sketchQuadratic(from, arc.control, to, hashSeed(key), DEFAULT_JITTER),
      middle: arc.middle,
      start: from,
      end: to,
      edge,
      dash,
      arrow: !communication,
      label: communication ? VIEW_COPY.communicates : null,
      endpoint,
    })
  }
  for (const edge of props.links.manualEdges) {
    const source = nodeCenter(edge.from)
    const target = nodeCenter(edge.to)
    if (!source || !target) continue
    const dash = edge.communication ? COMMUNICATION_DASH : edge.kind === 'async' ? ASYNC_DASH : undefined
    add(lineKey(edge), borderPoint(source, target, halfOf(edge.from)), borderPoint(target, source, halfOf(edge.to)), edge, dash, edge.communication === true)
  }
  const { start, end } = layout.value
  const radius = FLOW_GEOMETRY.endpointRadius
  const startPlacement = props.links.startAgent ? placementOf.value.get(props.links.startAgent) : undefined
  if (startPlacement) {
    const origin = { x: start.x, y: start.y + radius }
    add(START_LINE_KEY, origin, borderPoint(startPlacement, origin), null, undefined, false, true)
  }
  for (const agent of props.links.endAgents) {
    const placement = nodeCenter(agent)
    if (!placement) continue
    const target = { x: end.x, y: end.y - radius }
    add(endLineKey(agent), borderPoint(placement, target, halfOf(agent)), target, null, undefined, false, true)
  }
  return result
})

// Barra da linha selecionada: tipo (síncrona ou assíncrona) e remoção. Só para linhas manuais.
const toolbar = computed(() => {
  const line = lines.value.find((candidate) => candidate.key === props.selectedLine)
  if (!line) return null
  const edge = line.edge
  const keys = edge ? (['sync', 'async', 'remove'] as const) : (['remove'] as const)
  const totalWidth = keys.reduce((sum, key) => sum + TOOLBAR_WIDTHS[key], 0) + TOOLBAR_GAP * (keys.length - 1)
  let x = line.middle.x - totalWidth / 2
  const buttons = keys.map((key) => {
    const button = { key, x, width: TOOLBAR_WIDTHS[key], active: key !== 'remove' && edge?.kind === key }
    x += TOOLBAR_WIDTHS[key] + TOOLBAR_GAP
    return button
  })
  return { key: line.key, edge, y: line.middle.y + TOOLBAR_OFFSET, buttons }
})

const TOOLBAR_LABELS = {
  sync: VIEW_COPY.lineSync,
  async: VIEW_COPY.lineAsync,
  remove: VIEW_COPY.removeArrow,
} as const

function linePath(from: FlowPoint, to: FlowPoint): string {
  return `M ${from.x} ${from.y} L ${to.x} ${to.y}`
}

function isLeader(agent: Agent): boolean {
  return props.links.startAgent === agent
}

function isHighlighted(agent: Agent): boolean {
  return props.selectedAgent === agent || props.groupSelected.includes(agent)
}

// Bolinhas de conexão só aparecem com o mouse sobre o agente ou depois de clicar nele.
// Enquanto a ligação é arrastada, as bolinhas do nó de origem ficam: o ponteiro sai do nó no meio do arraste.
function showPorts(agent: Agent): boolean {
  return hoveredAgent.value === agent || isHighlighted(agent) || linkDrag.value?.from === agent
}

function isLineSelected(line: Line): boolean {
  return props.selectedLine === line.key
}

function nodeLeft(x: number): number {
  return x - FLOW_GEOMETRY.nodeWidth / 2
}

function nodeTop(y: number): number {
  return y - FLOW_GEOMETRY.nodeHeight / 2
}

function roleOf(agent: Agent): string {
  return roster.profileOf(agent)?.role ?? ''
}

function worktreesLabel(agent: Agent): string {
  return String(props.configOf(agent).worktrees)
}

// Coordenadas do desenho (já descontando zoom e pan): o getScreenCTM leva o viewBox em conta.
function canvasPoint(clientX: number, clientY: number): FlowPoint | null {
  const svg = svgRef.value
  const matrix = svg?.getScreenCTM()
  if (!svg || !matrix) return null
  const point = svg.createSVGPoint()
  point.x = clientX
  point.y = clientY
  const local = point.matrixTransform(matrix.inverse())
  return { x: local.x, y: local.y }
}

function beginLinkDrag(from: string, start: FlowPoint, event: PointerEvent, communication = false): void {
  event.preventDefault()
  event.stopPropagation()
  linkDrag.value = { from, communication, start, pointer: start }
  window.addEventListener('pointermove', onLinkMove)
  window.addEventListener('pointerup', onLinkEnd)
  window.addEventListener('pointercancel', stopLink)
}

// Alt pressionado num conector lateral de agente começa uma ligação de comunicação.
function startLink(from: Agent, side: PortSide, event: PointerEvent): void {
  const origin = nodeCenter(from)
  if (!origin) return
  const communication = event.altKey && !gatewayById.value.has(from) && (side === 'left' || side === 'right')
  beginLinkDrag(from, portAt(origin, side, halfOf(from)), event, communication)
}

// A ligação do início sai de uma bolinha abaixo dele e termina num agente.
function startFromStart(event: PointerEvent): void {
  const { start } = layout.value
  beginLinkDrag(FLOW_START, { x: start.x, y: start.y + FLOW_GEOMETRY.endpointRadius + PORT_RADIUS }, event)
}

function overEnd(point: FlowPoint): boolean {
  const { end } = layout.value
  return Math.hypot(point.x - end.x, point.y - end.y) <= FLOW_GEOMETRY.endpointRadius + PORT_RADIUS
}

function onLinkMove(event: PointerEvent): void {
  const point = canvasPoint(event.clientX, event.clientY)
  if (linkDrag.value && point) linkDrag.value = { ...linkDrag.value, pointer: point }
}

function onLinkEnd(event: PointerEvent): void {
  const current = linkDrag.value
  const point = canvasPoint(event.clientX, event.clientY)
  stopLink()
  if (!current || !point) return
  const target = nodeAt(point)
  if (current.from === FLOW_START) {
    if (target !== null) emit('linkStart', target)
    return
  }
  if (current.communication) {
    const fromCenter = nodeCenter(current.from)
    const toCenter = target === null ? undefined : nodeCenter(target)
    if (target !== null && fromCenter && toCenter) emit('linkCommunication', current.from, target, fromCenter.y, toCenter.y)
    return
  }
  if (target !== null) emit('link', current.from, target)
  else if (overEnd(point)) emit('linkEnd', current.from)
}

function stopLink(): void {
  window.removeEventListener('pointermove', onLinkMove)
  window.removeEventListener('pointerup', onLinkEnd)
  window.removeEventListener('pointercancel', stopLink)
  linkDrag.value = null
}

function beginAction(next: PointerAction): void {
  action = next
  window.addEventListener('pointermove', onActionMove)
  window.addEventListener('pointerup', onActionEnd)
  window.addEventListener('pointercancel', stopAction)
}

function startNodeDrag(agent: Agent, event: PointerEvent): void {
  if (event.button !== PRIMARY_BUTTON) return
  const world = canvasPoint(event.clientX, event.clientY)
  const center = props.positions.get(agent)
  if (!world || !center) return
  beginAction({
    kind: 'node',
    agent,
    offset: { x: world.x - center.x, y: world.y - center.y },
    moved: false,
    additive: event.shiftKey,
    clientX: event.clientX,
    clientY: event.clientY,
  })
}

function startEndpointDrag(which: 'start' | 'end', event: PointerEvent): void {
  if (event.button !== PRIMARY_BUTTON) return
  const world = canvasPoint(event.clientX, event.clientY)
  const center = layout.value[which]
  if (!world) return
  beginAction({
    kind: 'endpoint',
    which,
    offset: { x: world.x - center.x, y: world.y - center.y },
    moved: false,
    clientX: event.clientX,
    clientY: event.clientY,
  })
}

// A bolinha do meio de uma linha selecionada muda a curvatura arrastando.
function startCurveDrag(line: Line, event: PointerEvent): void {
  if (event.button !== PRIMARY_BUTTON) return
  event.stopPropagation()
  beginAction({ kind: 'curve', key: line.key, start: line.start, end: line.end })
}

function beginMarquee(event: PointerEvent): void {
  const world = canvasPoint(event.clientX, event.clientY)
  if (!world) return
  beginAction({
    kind: 'marquee',
    start: world,
    additive: event.shiftKey,
    moved: false,
    clientX: event.clientX,
    clientY: event.clientY,
  })
}

function beginPan(event: PointerEvent): void {
  beginAction({
    kind: 'pan',
    touch: false,
    moved: false,
    startX: event.clientX,
    startY: event.clientY,
    lastX: event.clientX,
    lastY: event.clientY,
  })
}

// Dois dedos sobre o fundo viram pan pelo ponto médio; um dedo faz o quadrado de seleção.
function beginTouchPan(): void {
  const middle = touchMiddle()
  if (!middle) return
  marqueeRect.value = null
  beginAction({
    kind: 'pan',
    touch: true,
    moved: true,
    startX: middle.x,
    startY: middle.y,
    lastX: middle.x,
    lastY: middle.y,
  })
}

function touchMiddle(): FlowPoint | null {
  const points = [...touchPoints.values()]
  if (points.length === 0) return null
  return {
    x: points.reduce((sum, point) => sum + point.x, 0) / points.length,
    y: points.reduce((sum, point) => sum + point.y, 0) / points.length,
  }
}

// Sobre o fundo: arrastar faz o quadrado de seleção (Shift soma ao grupo); espaço + arrasto, botão do meio e
// dois dedos movem a visão.
function onBackgroundDown(event: PointerEvent): void {
  if (event.button === PAN_BUTTON) {
    event.preventDefault()
    beginPan(event)
    return
  }
  if (event.pointerType === 'touch') {
    touchPoints.set(event.pointerId, { x: event.clientX, y: event.clientY })
    if (touchPoints.size >= TWO_FINGERS) beginTouchPan()
    else beginMarquee(event)
    return
  }
  if (event.button !== PRIMARY_BUTTON) return
  if (spacePressed) beginPan(event)
  else beginMarquee(event)
}

function onActionMove(event: PointerEvent): void {
  const current = action
  if (!current) return
  if (current.kind === 'pan') {
    const point = current.touch ? touchMiddle() : { x: event.clientX, y: event.clientY }
    if (!point) return
    if (!current.moved && Math.hypot(point.x - current.startX, point.y - current.startY) <= DRAG_THRESHOLD_PX) return
    current.moved = true
    panBy(point.x - current.lastX, point.y - current.lastY)
    current.lastX = point.x
    current.lastY = point.y
    return
  }
  if (current.kind === 'curve') {
    const world = canvasPoint(event.clientX, event.clientY)
    if (world) emit('bend', current.key, bendFromHandle(current.start, current.end, world))
    return
  }
  if (!current.moved && Math.hypot(event.clientX - current.clientX, event.clientY - current.clientY) <= DRAG_THRESHOLD_PX) return
  current.moved = true
  const world = canvasPoint(event.clientX, event.clientY)
  if (!world) return
  if (current.kind === 'marquee') {
    marqueeRect.value = { from: current.start, to: world }
    return
  }
  if (current.kind === 'endpoint') {
    emit('moveEndpoint', current.which, { x: world.x - current.offset.x, y: world.y - current.offset.y })
    return
  }
  moveDraggedNode(current, world, event.shiftKey)
}

// Arrasta o nó; se ele estiver no grupo selecionado, o grupo inteiro anda com o mesmo deslocamento.
function moveDraggedNode(current: Extract<PointerAction, { kind: 'node' }>, world: FlowPoint, snap: boolean): void {
  let target = { x: world.x - current.offset.x, y: world.y - current.offset.y }
  if (snap) {
    const others = layout.value.nodes.filter((node) => node.agent !== current.agent)
    target = snapPoint(target, others)
  }
  const at = props.positions.get(current.agent)
  if (!at) return
  const dx = target.x - at.x
  const dy = target.y - at.y
  if (dx === 0 && dy === 0) return
  const group = props.groupSelected.includes(current.agent) ? [...props.groupSelected] : [current.agent]
  emit('moveNodes', group, dx, dy)
}

// Deslocamento da visão em unidades do desenho: 1px de tela vale viewSize/largura do svg.
function panBy(clientDx: number, clientDy: number): void {
  const svg = svgRef.value
  if (!svg) return
  const rect = svg.getBoundingClientRect()
  zoomTouched = true
  view.value = clampView(
    {
      zoom: view.value.zoom,
      x: view.value.x - clientDx / (rect.width / viewWidth.value),
      y: view.value.y - clientDy / (rect.height / viewSize.value),
    },
    aspect.value,
  )
}

function onActionEnd(event: PointerEvent): void {
  const current = action
  stopAction()
  if (!current) return
  if (current.kind !== 'pan') keepContentInView()
  if (current.kind === 'node' && !current.moved) {
    emit('select', current.agent, current.additive)
    return
  }
  if (current.kind === 'endpoint' && !current.moved) {
    emit('selectEndpoint', current.which)
    return
  }
  if (current.kind === 'pan') return
  if (current.kind !== 'marquee') return
  const world = canvasPoint(event.clientX, event.clientY)
  marqueeRect.value = null
  if (!current.moved) {
    if (!current.additive) emit('clearSelection')
    return
  }
  if (world) emit('marquee', agentsInside(layout.value.nodes, current.start, world))
}

function stopAction(): void {
  window.removeEventListener('pointermove', onActionMove)
  window.removeEventListener('pointerup', onActionEnd)
  window.removeEventListener('pointercancel', stopAction)
  action = null
  marqueeRect.value = null
}

// Zoom com a roda, ancorado no cursor; dentro da faixa de zoom da área fixa.
function onWheel(event: WheelEvent): void {
  event.preventDefault()
  const anchor = canvasPoint(event.clientX, event.clientY)
  if (!anchor) return
  zoomTouched = true
  view.value = zoomAt(view.value, event.deltaY < 0 ? ZOOM_STEP : 1 / ZOOM_STEP, anchor, aspect.value)
}

function zoomFromCentre(factor: number): void {
  const current = view.value
  zoomTouched = true
  view.value = zoomAt(
    current,
    factor,
    { x: current.x + viewWidth.value / 2, y: current.y + viewSize.value / 2 },
    aspect.value,
  )
}

// Volta a enquadrar nós, início e fim, como no zoom inicial.
function recentre(): void {
  zoomTouched = false
  view.value = frameContent(layout.value, aspect.value)
}

// Só reenquadra fora do arraste: durante o gesto a visão fica parada, e ao soltar ou adicionar agente ela acompanha o desenho.
function keepContentInView(): void {
  if (!contentFitsView(layout.value, view.value, aspect.value)) view.value = frameContent(layout.value, aspect.value)
}

watch(layout, () => {
  if (action === null) keepContentInView()
})

function onToolbar(toolbarState: { key: string; edge: FlowEdge | null }, key: 'sync' | 'async' | 'remove'): void {
  if (key === 'remove') {
    if (toolbarState.edge) emit('unlink', toolbarState.edge)
    else emit('removeLine', toolbarState.key)
    return
  }
  if (toolbarState.edge) emit('setKind', toolbarState.edge, key)
}

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  return target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)
}

// Espaço segurado vira pan; a tecla não rola a página nem ativa botões enquanto o canvas está em foco.
function onSpaceDown(event: KeyboardEvent): void {
  if (event.code !== 'Space' || isTypingTarget(event.target)) return
  event.preventDefault()
  spacePressed = true
}

function onSpaceUp(event: KeyboardEvent): void {
  if (event.code === 'Space') spacePressed = false
}

function trackTouch(event: PointerEvent): void {
  if (event.pointerType !== 'touch') return
  if (event.type === 'pointerup' || event.type === 'pointercancel') {
    touchPoints.delete(event.pointerId)
    return
  }
  if (touchPoints.has(event.pointerId)) touchPoints.set(event.pointerId, { x: event.clientX, y: event.clientY })
}

function toCanvasPoint(clientX: number, clientY: number): FlowPoint | null {
  return canvasPoint(clientX, clientY)
}

defineExpose({ toCanvasPoint })

onMounted(() => {
  if (containerRef.value) resizeObserver.observe(containerRef.value)
  window.addEventListener('keydown', onSpaceDown)
  window.addEventListener('keyup', onSpaceUp)
  window.addEventListener('pointermove', trackTouch)
  window.addEventListener('pointerup', trackTouch)
  window.addEventListener('pointercancel', trackTouch)
})
onUnmounted(() => {
  cancelHoverClear()
  resizeObserver.disconnect()
  stopLink()
  stopAction()
  window.removeEventListener('keydown', onSpaceDown)
  window.removeEventListener('keyup', onSpaceUp)
  window.removeEventListener('pointermove', trackTouch)
  window.removeEventListener('pointerup', trackTouch)
  window.removeEventListener('pointercancel', trackTouch)
})
</script>

<template>
  <div data-flow-canvas class="flex min-h-0 w-full flex-col gap-2">
  <div ref="containerRef" class="relative min-h-0 w-full flex-1">
    <div aria-hidden="true" :class="DOT_GRID_CLASSES" />
    <div class="absolute right-2 top-2 z-10 flex gap-1">
      <UiButton variant="quiet" size="sm" :aria-label="VIEW_COPY.zoomIn" @press="zoomFromCentre(ZOOM_STEP)">+</UiButton>
      <UiButton variant="quiet" size="sm" :aria-label="VIEW_COPY.zoomOut" @press="zoomFromCentre(1 / ZOOM_STEP)">−</UiButton>
      <UiButton variant="quiet" size="sm" :aria-label="VIEW_COPY.recenter" :title="VIEW_COPY.recenter" @press="recentre()">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
          <circle cx="12" cy="12" r="7" />
          <path d="M12 2v4M12 18v4M2 12h4M18 12h4" />
        </svg>
      </UiButton>
    </div>
    <svg
      ref="svgRef"
      :viewBox="viewBox"
      role="group"
      :aria-label="VIEW_COPY.flowTitle"
      class="absolute inset-0 h-full w-full touch-none select-none"
      @wheel="onWheel"
    >
      <defs>
        <marker
          :id="ARROWHEAD_ID"
          viewBox="0 0 10 10"
          refX="9"
          refY="5"
          markerWidth="7"
          markerHeight="7"
          orient="auto-start-reverse"
        >
          <path d="M0 0 L10 5 L0 10 z" class="fill-accent" />
        </marker>
        <marker
          :id="ARROWHEAD_INK_ID"
          viewBox="0 0 10 10"
          refX="9"
          refY="5"
          markerWidth="7"
          markerHeight="7"
          orient="auto-start-reverse"
        >
          <path d="M0 0 L10 5 L0 10 z" class="fill-ink" />
        </marker>
      </defs>

      <rect
        :x="0"
        :y="0"
        :width="CANVAS_AREA"
        :height="CANVAS_AREA"
        fill="transparent"
        class="cursor-default"
        @pointerdown="onBackgroundDown"
      />

      <g
        role="img"
        :aria-label="VIEW_COPY.flowStart"
        class="cursor-grab"
        @pointerdown="startEndpointDrag('start', $event)"
        @pointerenter="hoveredEndpoint = 'start'"
        @pointerleave="hoveredEndpoint = hoveredEndpoint === 'start' ? null : hoveredEndpoint"
      >
        <title>{{ [VIEW_COPY.flowStart, ...(problems.get(FLOW_START) ?? [])].join(' — ') }}</title>
        <circle
          :cx="layout.start.x"
          :cy="layout.start.y"
          :r="FLOW_GEOMETRY.endpointRadius"
          :class="problems.has(FLOW_START) ? 'fill-danger' : 'fill-ink'"
        />
        <circle
          v-if="hoveredEndpoint === 'start' || linkDrag?.from === FLOW_START"
          :cx="layout.start.x"
          :cy="layout.start.y + FLOW_GEOMETRY.endpointRadius + PORT_RADIUS"
          :r="PORT_RADIUS"
          :stroke-width="PORT_STROKE_WIDTH"
          class="cursor-crosshair fill-surface stroke-accent"
          @pointerdown="startFromStart($event)"
        />
        <foreignObject
          :x="layout.start.x + FLOW_GEOMETRY.endpointRadius + EMBER_ICON_GAP"
          :y="layout.start.y - EMBER_ICON_SIZE / 2"
          :width="EMBER_ICON_SIZE"
          :height="EMBER_ICON_SIZE"
        >
          <AgentPortrait :who="DEFAULT_RECIPIENT" :size="EMBER_ICON_SIZE" />
        </foreignObject>
        <text
          v-if="hoveredEndpoint === 'start'"
          :x="layout.start.x - FLOW_GEOMETRY.endpointRadius - ENDPOINT_LABEL_GAP"
          :y="layout.start.y + 4"
          text-anchor="end"
          class="fill-ink text-small pointer-events-none"
        >
          {{ VIEW_COPY.flowStart }}
        </text>
      </g>
      <g
        role="img"
        :aria-label="VIEW_COPY.flowEnd"
        class="cursor-grab"
        @pointerdown="startEndpointDrag('end', $event)"
        @pointerenter="hoveredEndpoint = 'end'"
        @pointerleave="hoveredEndpoint = hoveredEndpoint === 'end' ? null : hoveredEndpoint"
      >
        <title>{{ [VIEW_COPY.flowEnd, ...(problems.get(FLOW_END) ?? [])].join(' — ') }}</title>
        <circle
          :cx="layout.end.x"
          :cy="layout.end.y"
          :r="FLOW_GEOMETRY.endpointRadius"
          :class="problems.has(FLOW_END) ? 'fill-danger/10 stroke-danger' : 'fill-surface stroke-ink'"
          stroke-width="2"
        />
        <circle :cx="layout.end.x" :cy="layout.end.y" :r="END_INNER_RADIUS" :class="problems.has(FLOW_END) ? 'fill-danger' : 'fill-ink'" />
        <foreignObject
          :x="layout.end.x + FLOW_GEOMETRY.endpointRadius + EMBER_ICON_GAP"
          :y="layout.end.y - EMBER_ICON_SIZE / 2"
          :width="EMBER_ICON_SIZE"
          :height="EMBER_ICON_SIZE"
        >
          <AgentPortrait :who="DEFAULT_RECIPIENT" :size="EMBER_ICON_SIZE" />
        </foreignObject>
        <text
          v-if="hoveredEndpoint === 'end'"
          :x="layout.end.x - FLOW_GEOMETRY.endpointRadius - ENDPOINT_LABEL_GAP"
          :y="layout.end.y + 4"
          text-anchor="end"
          class="fill-ink text-small pointer-events-none"
        >
          {{ VIEW_COPY.flowEnd }}
        </text>
      </g>

      <g v-if="emptyBox" role="img" :aria-label="VIEW_COPY.emptyFlowTitle">
        <path
          :d="linePath({ x: layout.start.x, y: layout.start.y + FLOW_GEOMETRY.endpointRadius }, { x: layout.start.x, y: emptyBox.top })"
          fill="none"
          :stroke-width="ARROW_WIDTH"
          :marker-end="`url(#${ARROWHEAD_ID})`"
          class="stroke-muted"
        />
        <path
          :d="linePath({ x: layout.start.x, y: emptyBox.top + emptyBox.height }, { x: layout.end.x, y: layout.end.y - FLOW_GEOMETRY.endpointRadius })"
          fill="none"
          :stroke-width="ARROW_WIDTH"
          :marker-end="`url(#${ARROWHEAD_ID})`"
          class="stroke-muted"
        />
        <rect
          :x="emptyBox.left"
          :y="emptyBox.top"
          :width="emptyBox.width"
          :height="emptyBox.height"
          :rx="NODE_RADIUS"
          class="fill-surface stroke-line"
          stroke-width="1.5"
        />
        <circle :cx="layout.start.x" :cy="emptyBox.top + EMPTY_ICON_DY" :r="EMPTY_ICON_RADIUS" fill="none" class="stroke-muted" stroke-width="1.5" />
        <path
          :d="`M ${layout.start.x} ${emptyBox.top + EMPTY_ICON_DY - EMPTY_ICON_ARM} V ${emptyBox.top + EMPTY_ICON_DY + EMPTY_ICON_ARM} M ${layout.start.x - EMPTY_ICON_ARM} ${emptyBox.top + EMPTY_ICON_DY} H ${layout.start.x + EMPTY_ICON_ARM}`"
          fill="none"
          class="stroke-muted"
          stroke-width="1.5"
          stroke-linecap="round"
        />
        <text :x="layout.start.x" :y="emptyBox.top + EMPTY_TITLE_DY" text-anchor="middle" class="fill-ink text-control font-semibold">
          {{ VIEW_COPY.emptyFlowTitle }}
        </text>
        <text
          v-for="(step, index) in EMPTY_STEPS"
          :key="step"
          :x="layout.start.x"
          :y="emptyBox.top + EMPTY_STEP_DYS[index]"
          text-anchor="middle"
          class="fill-muted text-small"
        >
          {{ index + 1 }}. {{ step }}
        </text>
      </g>

      <g v-for="line in lines" :key="line.key">
        <path
          :d="line.d"
          fill="none"
          :stroke-width="ARROW_HIT_WIDTH"
          class="cursor-pointer stroke-line opacity-0"
          @click="emit('selectLine', line.key)"
        />
        <path
          :d="line.sketch"
          fill="none"
          stroke-linecap="round"
          :stroke-width="isLineSelected(line) ? ARROW_SELECTED_WIDTH : line.endpoint ? ENDPOINT_ARROW_WIDTH : ARROW_WIDTH"
          :stroke-dasharray="line.dash"
          :marker-end="line.arrow ? `url(#${line.endpoint ? ARROWHEAD_INK_ID : ARROWHEAD_ID})` : undefined"
          :class="isLineSelected(line) ? 'stroke-ink-pen' : line.endpoint ? 'stroke-ink' : 'stroke-graphite'"
          pointer-events="none"
        />
        <text
          v-if="line.label"
          :x="line.middle.x"
          :y="line.middle.y + COMMUNICATION_LABEL_DY"
          text-anchor="middle"
          class="pointer-events-none fill-muted font-hand text-annotation"
        >
          {{ line.label }}
        </text>
        <circle
          v-if="isLineSelected(line)"
          :cx="line.middle.x"
          :cy="line.middle.y"
          :r="HANDLE_RADIUS"
          class="cursor-move fill-surface stroke-accent"
          stroke-width="1.5"
          @pointerdown="startCurveDrag(line, $event)"
        />
      </g>

      <g v-if="toolbar">
        <g
          v-for="button in toolbar.buttons"
          :key="button.key"
          role="button"
          tabindex="0"
          :aria-label="TOOLBAR_LABELS[button.key]"
          class="cursor-pointer outline-none focus-visible:opacity-80"
          @click="onToolbar(toolbar, button.key)"
          @keydown.enter.prevent="onToolbar(toolbar, button.key)"
          @keydown.space.prevent="onToolbar(toolbar, button.key)"
        >
          <rect
            :x="button.x"
            :y="toolbar.y"
            :width="button.width"
            :height="TOOLBAR_HEIGHT"
            :rx="TOOLBAR_HEIGHT / 2"
            :class="button.active ? 'fill-accent' : 'fill-surface stroke-line'"
            stroke-width="1.5"
          />
          <text
            :x="button.x + button.width / 2"
            :y="toolbar.y + TOOLBAR_HEIGHT / 2 + 4"
            text-anchor="middle"
            :class="button.active ? 'fill-surface text-small font-semibold' : 'fill-ink text-small'"
          >
            {{ TOOLBAR_LABELS[button.key] }}
          </text>
        </g>
      </g>

      <path
        v-if="linkDrag"
        :d="linkPreviewPath"
        fill="none"
        :stroke-width="ARROW_WIDTH"
        :stroke-dasharray="PREVIEW_DASH"
        :marker-end="`url(#${ARROWHEAD_ID})`"
        class="stroke-accent"
        pointer-events="none"
      />

      <rect
        v-if="marqueeBox"
        :x="marqueeBox.x"
        :y="marqueeBox.y"
        :width="marqueeBox.width"
        :height="marqueeBox.height"
        :fill-opacity="MARQUEE_OPACITY"
        :stroke-dasharray="MARQUEE_DASH"
        class="fill-accent stroke-accent"
        pointer-events="none"
      />

      <g
        v-for="node in layout.nodes"
        :key="node.agent"
        role="button"
        tabindex="0"
        :aria-label="roster.nameOf(node.agent)"
        class="cursor-grab outline-none focus-visible:opacity-80"
        @pointerdown="startNodeDrag(node.agent, $event)"
        @pointerenter="enterAgent(node.agent)"
        @pointerleave="leaveAgent(node.agent)"
        @keydown.enter.prevent="emit('select', node.agent, false)"
        @keydown.space.prevent="emit('select', node.agent, false)"
      >
        <title v-if="problems.has(node.agent)">{{ problems.get(node.agent)?.join(' — ') }}</title>
        <rect
          :x="nodeLeft(node.x) + SHADOW_OFFSET"
          :y="nodeTop(node.y) + SHADOW_OFFSET"
          :width="FLOW_GEOMETRY.nodeWidth"
          :height="FLOW_GEOMETRY.nodeHeight"
          :rx="NODE_RADIUS"
          class="fill-graphite-soft"
        />
        <rect
          :x="nodeLeft(node.x)"
          :y="nodeTop(node.y)"
          :width="FLOW_GEOMETRY.nodeWidth"
          :height="FLOW_GEOMETRY.nodeHeight"
          :rx="NODE_RADIUS"
          :class="problems.has(node.agent) ? 'fill-danger/10' : isHighlighted(node.agent) ? 'fill-mark-blue/40' : 'fill-surface'"
        />
        <path
          :d="nodeSketch(node.agent)"
          :transform="`translate(${nodeLeft(node.x)} ${nodeTop(node.y)})`"
          fill="none"
          stroke-linecap="round"
          :stroke-width="isHighlighted(node.agent) || isLeader(node.agent) ? NODE_SELECTED_WIDTH : NODE_STROKE_WIDTH"
          :class="
            problems.has(node.agent)
              ? 'stroke-danger'
              : isHighlighted(node.agent) || isLeader(node.agent)
                ? 'stroke-ink-pen'
                : 'stroke-graphite'
          "
          pointer-events="none"
        />
        <path
          v-if="problems.has(node.agent)"
          :d="NODE_ERROR_SCRIBBLE"
          :transform="`translate(${nodeLeft(node.x) + FLOW_GEOMETRY.nodeWidth - NODE_ERROR_SCRIBBLE_INSET} ${nodeTop(node.y) + NODE_ERROR_SCRIBBLE_TOP})`"
          fill="none"
          stroke-linecap="round"
          class="stroke-danger"
          :stroke-width="NODE_STROKE_WIDTH"
          pointer-events="none"
        />
        <foreignObject
          :x="nodeLeft(node.x) + PORTRAIT_INSET"
          :y="nodeTop(node.y) + (FLOW_GEOMETRY.nodeHeight - PORTRAIT_SIZE) / 2"
          :width="PORTRAIT_SIZE"
          :height="PORTRAIT_SIZE"
        >
          <AgentPortrait :who="node.agent" :size="PORTRAIT_SIZE" />
        </foreignObject>
        <text :x="nodeLeft(node.x) + TEXT_X_OFFSET" :y="nodeTop(node.y) + NAME_DY" class="fill-ink text-control font-semibold">
          {{ roster.nameOf(node.agent) }}
        </text>
        <text :x="nodeLeft(node.x) + TEXT_X_OFFSET" :y="nodeTop(node.y) + ROLE_DY" class="fill-muted text-small">
          {{ roleOf(node.agent) }}
        </text>
        <g :transform="`translate(${nodeLeft(node.x) + TEXT_X_OFFSET} ${nodeTop(node.y) + SUMMARY_DY - SUMMARY_ICON_SIZE / 2 - 1})`">
          <path :d="INSTANCES_ICON" fill="none" class="stroke-muted" stroke-width="1.2" />
          <text x="18" y="10.5" class="fill-ink font-mono text-tiny">{{ configOf(node.agent).instances }}</text>
        </g>
        <g
          v-if="isLeader(node.agent)"
          :transform="`translate(${nodeLeft(node.x) + TEXT_X_OFFSET + WORKTREES_OFFSET} ${nodeTop(node.y) + SUMMARY_DY - SUMMARY_ICON_SIZE / 2 - 1})`"
        >
          <path :d="WORKTREES_ICON" fill="none" class="stroke-muted" stroke-width="1.2" />
          <text x="18" y="10.5" class="fill-ink font-mono text-tiny">{{ worktreesLabel(node.agent) }}</text>
        </g>
        <g v-if="isLeader(node.agent)">
          <rect
            :x="nodeLeft(node.x) + FLOW_GEOMETRY.nodeWidth - LEADER_PILL_WIDTH + LEADER_PILL_OFFSET / 2"
            :y="nodeTop(node.y) - LEADER_PILL_OFFSET"
            :width="LEADER_PILL_WIDTH"
            :height="LEADER_PILL_HEIGHT"
            :rx="LEADER_PILL_RADIUS"
            class="fill-accent"
          />
          <text
            :x="nodeLeft(node.x) + FLOW_GEOMETRY.nodeWidth - LEADER_PILL_WIDTH / 2 + LEADER_PILL_OFFSET / 2"
            :y="nodeTop(node.y) - LEADER_PILL_OFFSET + LEADER_PILL_HEIGHT / 2 + 4"
            text-anchor="middle"
            class="fill-surface text-tiny font-semibold"
          >
            {{ VIEW_COPY.leaderBadge }}
          </text>
        </g>
        <template v-if="showPorts(node.agent)">
          <circle
            v-for="side in PORT_SIDES"
            :key="side"
            :cx="portAt(node, side).x"
            :cy="portAt(node, side).y"
            :r="PORT_RADIUS"
            :stroke-width="PORT_STROKE_WIDTH"
            class="cursor-crosshair fill-surface stroke-accent"
            @pointerdown="startLink(node.agent, side, $event)"
          />
        </template>
      </g>
      <g
        v-for="gateway in gatewayNodes"
        :key="gateway.id"
        role="button"
        tabindex="0"
        :aria-label="gateway.kind === 'paralelo' ? VIEW_COPY.gatewayParallel : VIEW_COPY.gatewayExclusive"
        class="cursor-grab outline-none focus-visible:opacity-80"
        @pointerdown="startNodeDrag(gateway.id, $event)"
        @pointerenter="enterAgent(gateway.id)"
        @pointerleave="leaveAgent(gateway.id)"
        @keydown.enter.prevent="emit('select', gateway.id, false)"
        @keydown.space.prevent="emit('select', gateway.id, false)"
      >
        <title v-if="problems.has(gateway.id)">{{ problems.get(gateway.id)?.join(' — ') }}</title>
        <polygon
          :points="`${gateway.x},${gateway.y - GATEWAY_HALF.height} ${gateway.x + GATEWAY_HALF.width},${gateway.y} ${gateway.x},${gateway.y + GATEWAY_HALF.height} ${gateway.x - GATEWAY_HALF.width},${gateway.y}`"
          :class="problems.has(gateway.id) ? 'fill-danger/10' : isHighlighted(gateway.id) ? 'fill-mark-blue/40' : 'fill-surface'"
        />
        <path
          :d="gatewaySketch(gateway.id)"
          :transform="`translate(${gateway.x - GATEWAY_HALF.width} ${gateway.y - GATEWAY_HALF.height})`"
          fill="none"
          stroke-linecap="round"
          :stroke-width="isHighlighted(gateway.id) ? NODE_SELECTED_WIDTH : NODE_STROKE_WIDTH"
          :class="problems.has(gateway.id) ? 'stroke-danger' : isHighlighted(gateway.id) ? 'stroke-ink-pen' : 'stroke-graphite'"
          pointer-events="none"
        />
        <path
          :d="
            gateway.kind === 'paralelo'
              ? `M ${gateway.x - GATEWAY_ICON_ARM} ${gateway.y} H ${gateway.x + GATEWAY_ICON_ARM} M ${gateway.x} ${gateway.y - GATEWAY_ICON_ARM} V ${gateway.y + GATEWAY_ICON_ARM}`
              : `M ${gateway.x - GATEWAY_ICON_ARM + 1} ${gateway.y - GATEWAY_ICON_ARM + 1} L ${gateway.x + GATEWAY_ICON_ARM - 1} ${gateway.y + GATEWAY_ICON_ARM - 1} M ${gateway.x + GATEWAY_ICON_ARM - 1} ${gateway.y - GATEWAY_ICON_ARM + 1} L ${gateway.x - GATEWAY_ICON_ARM + 1} ${gateway.y + GATEWAY_ICON_ARM - 1}`
          "
          fill="none"
          class="stroke-ink"
          stroke-width="2"
          stroke-linecap="round"
          pointer-events="none"
        />
        <template v-if="showPorts(gateway.id)">
          <circle
            v-for="side in PORT_SIDES"
            :key="side"
            :cx="portAt(gateway, side, GATEWAY_HALF).x"
            :cy="portAt(gateway, side, GATEWAY_HALF).y"
            :r="PORT_RADIUS"
            :stroke-width="PORT_STROKE_WIDTH"
            class="cursor-crosshair fill-surface stroke-accent"
            @pointerdown="startLink(gateway.id, side, $event)"
          />
        </template>
      </g>
    </svg>
  </div>
  <ul :aria-label="VIEW_COPY.lineLegend" class="m-0 flex list-none flex-wrap gap-x-4 gap-y-1 px-1 text-tiny text-muted">
    <li v-for="sample in LEGEND_SAMPLES" :key="sample.label" class="flex items-center gap-2">
      <svg :width="LEGEND_SAMPLE_WIDTH" height="8" aria-hidden="true">
        <line
          x1="0"
          :y1="LEGEND_SAMPLE_Y"
          :x2="LEGEND_SAMPLE_WIDTH"
          :y2="LEGEND_SAMPLE_Y"
          :stroke-dasharray="sample.dash"
          :stroke-width="sample.ink ? ENDPOINT_ARROW_WIDTH : 1.5"
          :class="sample.ink ? 'stroke-ink' : 'stroke-accent'"
        />
      </svg>
      {{ sample.label }}
    </li>
  </ul>
  </div>
</template>
