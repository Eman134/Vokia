<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import MarkdownText from '../team/MarkdownText.vue'
import { useProjects } from '../projects/useProjects'
import { useLearningSuggestions } from './useLearningSuggestions'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import {
  DEFAULT_JITTER,
  sketchDiamond,
  sketchEllipse,
  sketchQuadratic,
  sketchRect,
} from '../../ui/sketch/sketchPath'
import {
  getContextGraph,
  type ContextGraph,
  type ContextGraphEdge,
  type ContextGraphNode,
  type ContextGraphNodeKind,
  type ContextGraphOptimization,
  type ContextGraphOptimizationKind,
} from './learningApi'
import { GRAPH_DESCRIPTION, GRAPH_EMPTY_TITLE } from './memoriaSample'

interface SimNode {
  node: ContextGraphNode
  x: number
  y: number
  vx: number
  vy: number
  degree: number
  radius: number
}

interface PositionedEdge {
  edge: ContextGraphEdge
  d: string
}

const KIND_LABEL: Record<ContextGraphNodeKind, string> = {
  decision: 'Decisão',
  keyword: 'Palavra-chave',
  memory: 'Memória',
  card: 'Card',
  file: 'Arquivo',
  artifact: 'Artefato',
}

const KIND_COLOR: Record<ContextGraphNodeKind, string> = {
  decision: 'var(--color-mark-pink)',
  keyword: 'var(--color-mark-blue)',
  memory: 'var(--color-mark-green)',
  card: 'var(--color-mark-yellow)',
  file: 'var(--color-accent-soft)',
  artifact: 'var(--color-warn-soft)',
}

const BADGE_CLASSES: Record<ContextGraphOptimizationKind, string> = {
  conflict: 'bg-warn-soft text-warn-text',
  context: 'bg-accent-soft text-accent',
  artifact: 'bg-accent-soft text-accent',
  order: 'bg-surface-2 text-muted',
}

const projects = useProjects()
const learning = useLearningSuggestions()

const graph = ref<ContextGraph | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const selectedNodeId = ref<string | null>(null)
const hoveredNodeId = ref<string | null>(null)
const activeKindFilter = ref<ContextGraphNodeKind | 'all'>('all')
const expandedOptId = ref<string | null>(null)

// Pan, zoom & node drag state for the organic constellation view
const svgRef = ref<SVGSVGElement | null>(null)
const graphGroupRef = ref<SVGGElement | null>(null)
const zoom = ref(1)
const panX = ref(0)
const panY = ref(0)
const isPanning = ref(false)
const panStart = ref({ x: 0, y: 0 })
const draggedPositions = ref<Record<string, { x: number; y: number }>>({})
const draggingNodeId = ref<string | null>(null)
const dragOffset = ref({ x: 0, y: 0 })
const dragStartClient = ref({ x: 0, y: 0 })
const didDragNode = ref(false)

async function loadGraph(projectId: number | undefined): Promise<void> {
  if (projectId === undefined) {
    graph.value = null
    return
  }
  loading.value = true
  error.value = null
  try {
    graph.value = await getContextGraph(projectId)
    selectedNodeId.value = null
    expandedOptId.value = null
    draggedPositions.value = {}
    resetView()
  } catch (err) {
    error.value = `Não foi possível montar o grafo: ${String(err)}`
    graph.value = null
  } finally {
    loading.value = false
  }
}

function resetView(): void {
  zoom.value = 1
  panX.value = 0
  panY.value = 0
  draggedPositions.value = {}
}

function adjustZoom(delta: number): void {
  zoom.value = Math.min(2.4, Math.max(0.55, Number((zoom.value + delta).toFixed(2))))
}

function onWheel(e: WheelEvent): void {
  e.preventDefault()
  adjustZoom(e.deltaY < 0 ? 0.12 : -0.12)
}

function clientToGraphPoint(clientX: number, clientY: number): { x: number; y: number } | null {
  const svg = svgRef.value
  const group = graphGroupRef.value
  if (!svg || !group) return null
  const ctm = group.getScreenCTM()
  if (!ctm) return null
  const pt = svg.createSVGPoint()
  pt.x = clientX
  pt.y = clientY
  const transformed = pt.matrixTransform(ctm.inverse())
  return { x: transformed.x, y: transformed.y }
}

function startNodeDrag(e: PointerEvent, sim: SimNode): void {
  if (e.button !== 0) return
  const pt = clientToGraphPoint(e.clientX, e.clientY)
  if (!pt) return
  draggingNodeId.value = sim.node.id
  dragOffset.value = { x: sim.x - pt.x, y: sim.y - pt.y }
  dragStartClient.value = { x: e.clientX, y: e.clientY }
  didDragNode.value = false
}

function startPan(e: PointerEvent): void {
  if ((e.target as HTMLElement)?.closest('[data-graph-node]')) return
  isPanning.value = true
  panStart.value = { x: e.clientX - panX.value, y: e.clientY - panY.value }
}

function movePan(e: PointerEvent): void {
  if (draggingNodeId.value !== null) {
    const dx = e.clientX - dragStartClient.value.x
    const dy = e.clientY - dragStartClient.value.y
    if (!didDragNode.value && dx * dx + dy * dy > 16) {
      didDragNode.value = true
    }
    if (didDragNode.value) {
      const pt = clientToGraphPoint(e.clientX, e.clientY)
      if (pt) {
        draggedPositions.value = {
          ...draggedPositions.value,
          [draggingNodeId.value]: {
            x: pt.x + dragOffset.value.x,
            y: pt.y + dragOffset.value.y,
          },
        }
      }
    }
    return
  }
  if (!isPanning.value) return
  panX.value = e.clientX - panStart.value.x
  panY.value = e.clientY - panStart.value.y
}

function endPan(): void {
  draggingNodeId.value = null
  isPanning.value = false
}

function onNodeClick(id: string): void {
  if (didDragNode.value) {
    didDragNode.value = false
    return
  }
  toggleNode(id)
}

watch(
  () => projects.selected?.id,
  (projectId) => {
    void loadGraph(projectId)
  },
  { immediate: true },
)

watch(
  () => learning.memories.length,
  () => {
    if (projects.selected?.id !== undefined) {
      void loadGraph(projects.selected.id)
    }
  },
)

const visibleNodes = computed<ContextGraphNode[]>(() => {
  const all = graph.value?.nodes ?? []
  if (activeKindFilter.value === 'all') return all
  return all.filter((n) => n.kind === activeKindFilter.value)
})

const visibleNodeIds = computed(() => new Set(visibleNodes.value.map((n) => n.id)))

const visibleEdges = computed<ContextGraphEdge[]>(() => {
  const ids = visibleNodeIds.value
  return (graph.value?.edges ?? []).filter((e) => ids.has(e.source) && ids.has(e.target))
})

// Deterministic force-directed constellation layout (Obsidian / InfraNodus style)
const simulatedNodes = computed<SimNode[]>(() => {
  const nodes = visibleNodes.value
  const edges = visibleEdges.value
  if (nodes.length === 0) return []

  const degreeMap = new Map<string, number>()
  for (const edge of edges) {
    degreeMap.set(edge.source, (degreeMap.get(edge.source) ?? 0) + 1)
    degreeMap.set(edge.target, (degreeMap.get(edge.target) ?? 0) + 1)
  }

  const W = 800
  const H = 500
  const cx = W / 2
  const cy = H / 2
  const spread = Math.max(1, Math.sqrt(nodes.length / 10))
  const minX = cx - 330 * spread
  const maxX = cx + 330 * spread
  const minY = cy - 195 * spread
  const maxY = cy + 195 * spread

  const sim: SimNode[] = nodes.map((node, i) => {
    const angle = (2 * Math.PI * i) / Math.max(1, nodes.length)
    const ring =
      (node.kind === 'card'
        ? 110
        : node.kind === 'keyword'
          ? 165
          : node.kind === 'decision' || node.kind === 'memory'
            ? 200
            : 235) * Math.min(1.6, spread)
    const degree = degreeMap.get(node.id) ?? 0
    return {
      node,
      x: cx + Math.cos(angle) * ring,
      y: cy + Math.sin(angle) * ring * 0.78,
      vx: 0,
      vy: 0,
      degree,
      radius: Math.min(18, 8 + Math.sqrt(degree) * 3.5),
    }
  })

  const indexById = new Map(sim.map((s, idx) => [s.node.id, idx]))
  const iterations = 130

  for (let step = 0; step < iterations; step++) {
    const cooling = 1 - step / (iterations + 12)

    // Strong repulsion so nodes spread across the viewport cleanly
    for (let i = 0; i < sim.length; i++) {
      for (let j = i + 1; j < sim.length; j++) {
        const a = sim[i]
        const b = sim[j]
        let dx = b.x - a.x
        let dy = b.y - a.y
        let distSq = dx * dx + dy * dy
        if (distSq < 1) {
          dx = 1
          dy = 1
          distSq = 2
        }
        const dist = Math.sqrt(distSq)
        const minSep = 125
        const sepBoost = dist < minSep ? (minSep - dist) * 0.3 : 0
        const force = ((14500 / distSq) + sepBoost) * cooling
        const fx = (dx / dist) * force
        const fy = (dy / dist) * force
        a.vx -= fx
        a.vy -= fy
        b.vx += fx
        b.vy += fy
      }
    }

    // Spring attraction along edges
    for (const edge of edges) {
      const ia = indexById.get(edge.source)
      const ib = indexById.get(edge.target)
      if (ia === undefined || ib === undefined) continue
      const a = sim[ia]
      const b = sim[ib]
      const dx = b.x - a.x
      const dy = b.y - a.y
      const dist = Math.max(1, Math.sqrt(dx * dx + dy * dy))
      const ideal = 155
      const force = ((dist - ideal) * 0.045) * cooling
      const fx = (dx / dist) * force
      const fy = (dy / dist) * force
      a.vx += fx
      a.vy += fy
      b.vx -= fx
      b.vy -= fy
    }

    // Gentle center gravity so disconnected clusters stay framed
    for (const s of sim) {
      s.vx += (cx - s.x) * 0.008 * cooling
      s.vy += (cy - s.y) * 0.008 * cooling
      s.x = Math.max(minX, Math.min(maxX, s.x + s.vx * 0.65))
      s.y = Math.max(minY, Math.min(maxY, s.y + s.vy * 0.65))
      s.vx *= 0.55
      s.vy *= 0.55
    }
  }

  // Hard collision-resolution pass considering both node circles and label bounding boxes
  const halfWidth = (s: SimNode) => {
    const labelLen = Math.min(28, s.node.label.length)
    return Math.max(s.radius + 14, (labelLen * 6.2 + 20) / 2)
  }
  const halfHeight = (s: SimNode) => s.radius + 20

  for (let pass = 0; pass < 40; pass++) {
    let moved = false
    for (let i = 0; i < sim.length; i++) {
      for (let j = i + 1; j < sim.length; j++) {
        const a = sim[i]
        const b = sim[j]
        let dx = b.x - a.x
        let dy = b.y - a.y
        if (dx === 0 && dy === 0) {
          dx = (i % 2 === 0 ? 1 : -1) * 0.5
          dy = (j % 2 === 0 ? 1 : -1) * 0.5
        }
        const overlapX = halfWidth(a) + halfWidth(b) - Math.abs(dx)
        const overlapY = halfHeight(a) + halfHeight(b) - Math.abs(dy)
        if (overlapX > 0 && overlapY > 0) {
          moved = true
          if (overlapX < overlapY) {
            const push = (overlapX / 2) + 0.5
            const sign = dx >= 0 ? 1 : -1
            a.x -= sign * push
            b.x += sign * push
          } else {
            const push = (overlapY / 2) + 0.5
            const sign = dy >= 0 ? 1 : -1
            a.y -= sign * push
            b.y += sign * push
          }
        }
      }
    }
    if (!moved) break
  }

  return sim
})

const displayedNodes = computed<SimNode[]>(() => {
  const overrides = draggedPositions.value
  if (Object.keys(overrides).length === 0) return simulatedNodes.value
  return simulatedNodes.value.map((s) => {
    const pos = overrides[s.node.id]
    return pos ? { ...s, x: pos.x, y: pos.y } : s
  })
})

const simMap = computed(() => new Map(displayedNodes.value.map((s) => [s.node.id, s])))

function hashSeed(id: string): number {
  let h = 2166136261
  for (let i = 0; i < id.length; i += 1) {
    h ^= id.charCodeAt(i)
    h = Math.imul(h, 16777619)
  }
  return h >>> 0
}

function nodeSketchPath(sim: SimNode): string {
  const d = sim.radius * 2
  const seed = hashSeed(sim.node.id)
  if (sim.node.kind === 'decision') return sketchDiamond(d + 4, d + 4, seed, DEFAULT_JITTER * 0.9)
  if (sim.node.kind === 'artifact') return sketchRect(d + 2, d + 2, seed, DEFAULT_JITTER * 0.85)
  return sketchEllipse(d + 3, d + 3, seed, DEFAULT_JITTER * 0.9)
}

function nodeSketchOffset(sim: SimNode): number {
  if (sim.node.kind === 'decision') return sim.radius + 2
  if (sim.node.kind === 'artifact') return sim.radius + 1
  return sim.radius + 1.5
}

const positionedEdges = computed<PositionedEdge[]>(() => {
  const map = simMap.value
  const list: PositionedEdge[] = []
  for (const edge of visibleEdges.value) {
    const src = map.get(edge.source)
    const dst = map.get(edge.target)
    if (!src || !dst) continue
    const dx = dst.x - src.x
    const dy = dst.y - src.y
    const cx = (src.x + dst.x) / 2 - dy * 0.11
    const cy = (src.y + dst.y) / 2 + dx * 0.11
    list.push({
      edge,
      d: sketchQuadratic(
        { x: src.x, y: src.y },
        { x: cx, y: cy },
        { x: dst.x, y: dst.y },
        hashSeed(edge.id),
        DEFAULT_JITTER,
      ),
    })
  }
  return list
})

const focusNodeId = computed(() => hoveredNodeId.value ?? selectedNodeId.value)

const filterActive = computed(
  () => Boolean(learning.agentFilter) || Boolean(learning.searchQuery.trim()),
)

function nodeMatchesFilters(node: ContextGraphNode): boolean {
  const agent = learning.agentFilter
  const q = learning.searchQuery.trim().toLowerCase()
  const agentOk = !agent || node.agents.includes(agent)
  const qOk =
    !q ||
    `${node.label} ${node.subtitle ?? ''} ${node.path ?? ''} ${node.wing ?? ''} ${node.room ?? ''}`
      .toLowerCase()
      .includes(q)
  return agentOk && qOk
}

const highlightedIds = computed<Set<string>>(() => {
  const ids = new Set<string>()
  const active = focusNodeId.value
  if (active) {
    ids.add(active)
    for (const edge of visibleEdges.value) {
      if (edge.source === active) ids.add(edge.target)
      if (edge.target === active) ids.add(edge.source)
    }
    return ids
  }
  if (filterActive.value) {
    for (const node of visibleNodes.value) {
      if (nodeMatchesFilters(node)) ids.add(node.id)
    }
  }
  return ids
})

const selectedNode = computed<ContextGraphNode | null>(() => {
  if (!selectedNodeId.value) return null
  return graph.value?.nodes.find((n) => n.id === selectedNodeId.value) ?? null
})

const selectedNeighbors = computed<Array<{ node: ContextGraphNode; relation: string }>>(() => {
  const active = selectedNodeId.value
  if (!active || !graph.value) return []
  const out: Array<{ node: ContextGraphNode; relation: string }> = []
  const seen = new Set<string>()
  for (const edge of graph.value.edges) {
    const otherId = edge.source === active ? edge.target : edge.target === active ? edge.source : null
    if (!otherId || seen.has(otherId)) continue
    const nb = graph.value.nodes.find((n) => n.id === otherId)
    if (nb) {
      seen.add(otherId)
      out.push({ node: nb, relation: edge.kind })
    }
  }
  return out
})

const kindCounts = computed(() => {
  const counts: Record<ContextGraphNodeKind, number> = {
    decision: 0,
    keyword: 0,
    memory: 0,
    card: 0,
    file: 0,
    artifact: 0,
  }
  for (const n of graph.value?.nodes ?? []) {
    counts[n.kind] = (counts[n.kind] ?? 0) + 1
  }
  return counts
})

const hasNodes = computed(() => (graph.value?.nodes.length ?? 0) > 0)
const optimizations = computed<ContextGraphOptimization[]>(() => graph.value?.optimizations ?? [])

function toggleNode(id: string): void {
  selectedNodeId.value = selectedNodeId.value === id ? null : id
}

function toggleOptFiles(opt: ContextGraphOptimization): void {
  expandedOptId.value = expandedOptId.value === opt.id ? null : opt.id
  if (opt.files.length > 0) {
    selectedNodeId.value = `file:${opt.files[0]}`
  } else if (opt.cardIds.length > 0) {
    selectedNodeId.value = `card:${opt.cardIds[0]}`
  }
}

function edgeStroke(kind: ContextGraphEdge['kind']): string {
  if (kind === 'conflict_risk') return 'var(--color-warn)'
  if (kind === 'touches_file' || kind === 'about_card') return 'var(--color-ok)'
  if (kind === 'has_keyword') return 'var(--color-accent)'
  if (kind === 'superseded_by') return 'var(--color-muted)'
  return 'var(--color-graphite)'
}

function edgeOpacity(edge: ContextGraphEdge): number {
  const active = focusNodeId.value
  if (!active && !filterActive.value) return 0.65
  if (active) {
    return edge.source === active || edge.target === active ? 1 : 0.14
  }
  return highlightedIds.value.has(edge.source) || highlightedIds.value.has(edge.target) ? 0.95 : 0.15
}

function nodeOpacity(id: string): number {
  if (!focusNodeId.value && !filterActive.value) return 1
  return highlightedIds.value.has(id) ? 1 : 0.24
}

function nodeColor(node: ContextGraphNode): string {
  if (node.conflictRisk) return 'var(--color-warn-soft)'
  return KIND_COLOR[node.kind] ?? 'var(--color-mark-yellow)'
}
</script>

<template>
  <div class="flex flex-wrap items-baseline justify-between gap-2 px-4.5 py-4">
    <div class="flex flex-wrap items-baseline gap-2">
      <h2 class="text-lead text-ink">Grafo de Conhecimento</h2>
      <p class="text-caption text-muted">{{ GRAPH_DESCRIPTION }}</p>
    </div>
    <UiButton
      v-if="projects.selected"
      variant="quiet"
      size="sm"
      :disabled="loading"
      @press="loadGraph(projects.selected?.id)"
    >
      {{ loading ? 'Atualizando...' : 'Atualizar' }}
    </UiButton>
  </div>

  <p v-if="error" class="px-4.5 pb-4 text-caption text-danger">
    {{ error }}
  </p>

  <UiEmpty
    v-else-if="!hasNodes && !loading"
    class="px-4.5 pb-6"
    :title="GRAPH_EMPTY_TITLE"
    text="Quando memórias, cards ativos, passos de trabalho ou artefatos existirem no projeto, o grafo aparece aqui."
  />

  <template v-else-if="hasNodes">
    <div class="flex flex-col border-t border-line lg:flex-row">
      <div
        class="relative isolate z-0 h-[500px] flex-1 overflow-hidden select-none"
        style="background-color: light-dark(#fcf6e3, #17171c);"
        :class="isPanning ? 'cursor-grabbing' : 'cursor-grab'"
        @wheel="onWheel"
        @pointerdown="startPan"
        @pointermove="movePan"
        @pointerup="endPan"
        @pointerleave="endPan"
      >
        <!-- Top-left cluster filter pills -->
        <div class="absolute left-3 top-3 z-[1] flex flex-wrap items-center gap-1 rounded-md border border-line bg-surface/95 p-1 shadow-seg backdrop-blur-sm">
          <button
            type="button"
            class="cursor-pointer rounded-sm px-2 py-0.5 text-tiny font-medium transition-colors"
            :class="activeKindFilter === 'all' ? 'bg-accent-soft text-accent' : 'text-muted hover:text-ink'"
            @click="activeKindFilter = 'all'"
          >
            Todos ({{ graph?.nodes.length ?? 0 }})
          </button>
          <button
            v-for="kind in (['decision', 'keyword', 'memory', 'card', 'file', 'artifact'] as const)"
            :key="kind"
            type="button"
            class="inline-flex cursor-pointer items-center gap-1.5 rounded-sm px-2 py-0.5 text-tiny font-medium transition-colors"
            :class="activeKindFilter === kind ? 'bg-accent-soft text-accent' : 'text-muted hover:text-ink'"
            @click="activeKindFilter = kind"
          >
            <span class="size-2.5 rounded-full border border-ink/40" :style="{ backgroundColor: KIND_COLOR[kind] }" />
            {{ KIND_LABEL[kind] }} ({{ kindCounts[kind] }})
          </button>
        </div>

        <!-- Bottom-left zoom controls -->
        <div class="absolute bottom-3 left-3 z-[1] flex items-center gap-1 rounded-md border border-line bg-surface/95 p-1 text-tiny text-ink shadow-seg backdrop-blur-sm">
          <button type="button" class="cursor-pointer rounded-sm px-2 py-0.5 hover:bg-hover" title="Diminuir zoom" @click="adjustZoom(-0.15)">−</button>
          <span class="px-1 tabular-nums text-muted">{{ Math.round(zoom * 100) }}%</span>
          <button type="button" class="cursor-pointer rounded-sm px-2 py-0.5 hover:bg-hover" title="Aumentar zoom" @click="adjustZoom(0.15)">+</button>
          <button type="button" class="cursor-pointer rounded-sm px-2 py-0.5 text-muted hover:bg-hover hover:text-ink" @click="resetView()">Reset</button>
        </div>

        <!-- Excalidraw Hand-Drawn SVG Constellation -->
        <svg
          ref="svgRef"
          viewBox="0 0 800 500"
          class="h-full w-full"
          preserveAspectRatio="xMidYMid meet"
        >
          <g ref="graphGroupRef" :transform="`translate(${400 + panX}, ${250 + panY}) scale(${zoom}) translate(-400, -250)`">
            <path
              v-for="item in positionedEdges"
              :key="item.edge.id"
              :d="item.d"
              :stroke="edgeStroke(item.edge.kind)"
              :stroke-width="focusNodeId && (item.edge.source === focusNodeId || item.edge.target === focusNodeId) ? 2.1 : 1.4"
              :stroke-dasharray="item.edge.kind === 'conflict_risk' ? '5 4' : item.edge.kind === 'superseded_by' ? '2 4' : undefined"
              :stroke-opacity="edgeOpacity(item.edge)"
              stroke-linecap="round"
              fill="none"
            />

            <g
              v-for="sim in displayedNodes"
              :key="sim.node.id"
              data-graph-node
              :class="draggingNodeId === sim.node.id ? 'cursor-grabbing' : 'cursor-grab'"
              class="transition-opacity duration-150"
              :style="{ opacity: nodeOpacity(sim.node.id) }"
              @pointerdown.stop="startNodeDrag($event, sim)"
              @mouseenter="hoveredNodeId = sim.node.id"
              @mouseleave="hoveredNodeId = null"
              @click.stop="onNodeClick(sim.node.id)"
            >
              <!-- Pastel hand-painted fill dab (slightly offset like Excalidraw hachure/marker) -->
              <g :transform="`translate(${sim.x - nodeSketchOffset(sim) + 1.5}, ${sim.y - nodeSketchOffset(sim) + 2})`">
                <path
                  :d="nodeSketchPath(sim)"
                  :fill="nodeColor(sim.node)"
                  fill-opacity="0.92"
                />
              </g>
              <!-- Hand-drawn pencil/ink outline -->
              <g :transform="`translate(${sim.x - nodeSketchOffset(sim)}, ${sim.y - nodeSketchOffset(sim)})`">
                <path
                  :d="nodeSketchPath(sim)"
                  fill="none"
                  :stroke="selectedNodeId === sim.node.id || hoveredNodeId === sim.node.id ? 'var(--color-accent)' : 'var(--color-ink)'"
                  :stroke-width="selectedNodeId === sim.node.id ? 2.1 : 1.45"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </g>
              <!-- Node label -->
              <text
                :x="sim.x"
                :y="sim.y + sim.radius + 17"
                text-anchor="middle"
                fill="var(--color-ink)"
                font-size="11.5"
                font-weight="600"
                class="pointer-events-none select-none"
                style="paint-order: stroke; stroke: light-dark(#fcf6e3, #17171c); stroke-width: 4.5px; stroke-linejoin: round;"
              >
                {{ sim.node.label.length > 28 ? `${sim.node.label.slice(0, 27)}…` : sim.node.label }}
              </text>
            </g>
          </g>
        </svg>
      </div>

      <!-- Collapsible Right Inspector Panel -->
      <aside
        v-if="selectedNode"
        class="flex w-full flex-col gap-3 border-t border-line bg-surface p-4 lg:w-80 lg:border-t-0 lg:border-l"
      >
        <div class="flex items-start justify-between gap-2">
          <div class="flex flex-wrap items-center gap-1.5">
            <span
              class="inline-flex items-center gap-1 rounded-sm border border-ink/20 px-1.5 py-0.5 text-tiny font-semibold text-mark-ink"
              :style="{ backgroundColor: nodeColor(selectedNode) }"
            >
              {{ KIND_LABEL[selectedNode.kind] }}
            </span>
            <span v-if="selectedNode.wing" class="rounded-sm bg-surface-2 px-1.5 py-0.5 text-tiny text-muted">
              {{ selectedNode.wing }}<template v-if="selectedNode.room"> / {{ selectedNode.room }}</template>
            </span>
          </div>
          <button
            type="button"
            class="cursor-pointer text-meta text-muted hover:text-ink"
            @click="selectedNodeId = null"
          >
            Fechar
          </button>
        </div>

        <h3 class="wrap-break-word text-label font-semibold text-ink">{{ selectedNode.label }}</h3>
        <MarkdownText
          v-if="selectedNode.subtitle || selectedNode.path"
          :text="(selectedNode.subtitle ?? selectedNode.path)!"
          class="wrap-break-word text-small text-muted"
        />

        <div v-if="selectedNode.agents.length > 0" class="flex flex-wrap items-center gap-1.5">
          <span class="text-tiny text-muted">Agentes:</span>
          <span
            v-for="agent in selectedNode.agents"
            :key="agent"
            class="inline-flex items-center gap-1 rounded-sm border border-line px-1.5 py-0.5 text-tiny text-ink"
          >
            <AgentPortrait :who="agent" :size="14" />
            {{ agent }}
          </span>
        </div>

        <div class="flex flex-col gap-1.5 border-t border-line pt-3">
          <span class="text-tiny font-semibold text-muted">
            Conexões ({{ selectedNeighbors.length }})
          </span>
          <p v-if="selectedNeighbors.length === 0" class="text-tiny text-muted">
            Nó isolado — sem arestas registradas ainda.
          </p>
          <div v-else class="flex max-h-56 flex-col gap-1 overflow-y-auto">
            <button
              v-for="item in selectedNeighbors"
              :key="item.node.id"
              type="button"
              class="flex cursor-pointer items-center justify-between gap-2 rounded-sm border border-line bg-surface-2 px-2 py-1 text-left text-tiny text-ink hover:border-accent"
              @click="selectedNodeId = item.node.id"
            >
              <span class="truncate font-medium">{{ item.node.label }}</span>
              <span class="shrink-0 text-muted">{{ item.relation }}</span>
            </button>
          </div>
        </div>
      </aside>
    </div>

    <div class="flex flex-col gap-3 border-t border-line px-4.5 py-4">
      <h3 class="text-label font-semibold text-ink">Otimizações sugeridas</h3>
      <p v-if="optimizations.length === 0" class="text-small text-muted">
        Nenhuma otimização pendente para o estado atual do grafo.
      </p>
      <ul v-else class="flex flex-col gap-2">
        <li
          v-for="opt in optimizations"
          :key="opt.id"
          class="flex flex-col gap-2 rounded-md border border-line bg-surface-2 px-3 py-2.5"
        >
          <div class="flex flex-wrap items-center gap-2">
            <span class="rounded-sm px-1.5 py-0.5 text-tiny font-semibold" :class="BADGE_CLASSES[opt.kind]">
              {{ opt.badge }}
            </span>
            <MarkdownText :text="opt.message" class="min-w-0 flex-1 text-small text-ink" />
            <UiButton
              v-if="opt.files.length > 0 || opt.cardIds.length > 0"
              variant="quiet"
              size="sm"
              @press="toggleOptFiles(opt)"
            >
              {{ expandedOptId === opt.id ? 'Ocultar' : 'Focar no grafo' }}
            </UiButton>
          </div>
          <div v-if="expandedOptId === opt.id && opt.files.length > 0" class="flex flex-wrap gap-1.5">
            <span
              v-for="file in opt.files"
              :key="file"
              class="rounded-sm border border-line bg-surface px-2 py-0.5 font-mono text-tiny text-muted"
            >
              {{ file }}
            </span>
          </div>
        </li>
      </ul>
    </div>
  </template>
</template>
