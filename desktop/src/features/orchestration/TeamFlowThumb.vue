<script setup lang="ts">
import { computed } from 'vue'
import type { Agent } from '../team/events'
import { DEFAULT_JITTER, sketchQuadratic, sketchRect } from '../../ui/sketch/sketchPath'
import { VIEW_COPY } from './orchestrationLabels'
import { arcPath, borderPoint, canvasLayout, contentBounds, FLOW_GEOMETRY, hashSeed, type FlowPoint } from './flowGraph'
import type { FlowDraft } from './useFlowDraft'

const props = defineProps<{ draft: FlowDraft | undefined }>()

const THUMB_PADDING = 12
const THUMB_STROKE_WIDTH = 2
const THUMB_CORNER_RADIUS = 14
// Mesmo gerador do diagrama, com metade do tremor: a miniatura é pequena e o traço grosso poluiria.
const THUMB_JITTER = DEFAULT_JITTER / 2

// Só desenha quem já tem agentes no rascunho; time nunca aberto mostra o texto de vazio, sem desenho inventado.
const drawing = computed(() => (props.draft && props.draft.agents.value.length > 0 ? props.draft : null))
const layout = computed(() => {
  const draft = drawing.value
  return draft ? canvasLayout(draft.agents.value, draft.positions, draft.endpoints) : null
})
const viewBox = computed(() => {
  if (layout.value === null) return ''
  const bounds = contentBounds(layout.value)
  const width = bounds.right - bounds.left + 2 * THUMB_PADDING
  const height = bounds.bottom - bounds.top + 2 * THUMB_PADDING
  return `${bounds.left - THUMB_PADDING} ${bounds.top - THUMB_PADDING} ${width} ${height}`
})

const arrows = computed(() => {
  const draft = drawing.value
  if (draft === null || layout.value === null) return []
  const placements = new Map(layout.value.nodes.map((node) => [node.agent, node]))
  const { start, end } = layout.value
  const radius = FLOW_GEOMETRY.endpointRadius
  const { manualEdges, startAgent, endAgents } = draft.completed.value
  const paths: string[] = []
  const sketch = (from: FlowPoint, to: FlowPoint, id: string) => {
    const arc = arcPath(from, to)
    paths.push(sketchQuadratic(from, arc.control, to, hashSeed(id), THUMB_JITTER))
  }
  for (const edge of manualEdges) {
    const source = placements.get(edge.from)
    const target = placements.get(edge.to)
    if (source && target) sketch(borderPoint(source, target), borderPoint(target, source), `${edge.from}>${edge.to}`)
  }
  const origin: FlowPoint = { x: start.x, y: start.y + radius }
  const startPlacement = startAgent ? placements.get(startAgent) : undefined
  if (startPlacement) sketch(origin, borderPoint(startPlacement, origin), `inicio>${startAgent}`)
  for (const agent of endAgents) {
    const placement = placements.get(agent)
    if (placement) sketch(borderPoint(placement, end), { x: end.x, y: end.y - radius }, `${agent}>fim`)
  }
  return paths
})

const nodeSketches = computed(() => {
  const sketches = new Map<string, string>()
  for (const node of layout.value?.nodes ?? []) {
    sketches.set(
      node.agent,
      sketchRect(FLOW_GEOMETRY.nodeWidth, FLOW_GEOMETRY.nodeHeight, hashSeed(node.agent), THUMB_JITTER),
    )
  }
  return sketches
})

function isLeader(agent: Agent): boolean {
  return drawing.value?.completed.value.startAgent === agent
}
</script>

<template>
  <span class="flex h-28 w-full items-center justify-center overflow-hidden rounded-md bg-surface-2 p-2">
    <svg
      v-if="layout"
      :viewBox="viewBox"
      preserveAspectRatio="xMidYMid meet"
      class="h-full w-full"
      aria-hidden="true"
    >
      <path v-for="(d, index) in arrows" :key="index" :d="d" fill="none" stroke-linecap="round" stroke-width="2" class="stroke-graphite" />
      <g v-for="node in layout.nodes" :key="node.agent">
        <rect
          :x="node.x - FLOW_GEOMETRY.nodeWidth / 2"
          :y="node.y - FLOW_GEOMETRY.nodeHeight / 2"
          :width="FLOW_GEOMETRY.nodeWidth"
          :height="FLOW_GEOMETRY.nodeHeight"
          :rx="THUMB_CORNER_RADIUS"
          class="fill-surface"
        />
        <path
          :d="nodeSketches.get(node.agent)"
          :transform="`translate(${node.x - FLOW_GEOMETRY.nodeWidth / 2} ${node.y - FLOW_GEOMETRY.nodeHeight / 2})`"
          fill="none"
          stroke-linecap="round"
          :class="isLeader(node.agent) ? 'stroke-ink-pen' : 'stroke-graphite'"
          :stroke-width="THUMB_STROKE_WIDTH"
        />
      </g>
      <circle :cx="layout.start.x" :cy="layout.start.y" :r="FLOW_GEOMETRY.endpointRadius" class="fill-ink" />
      <circle :cx="layout.end.x" :cy="layout.end.y" :r="FLOW_GEOMETRY.endpointRadius" class="fill-surface stroke-ink" stroke-width="2" />
    </svg>
    <span v-else class="text-small text-faint">{{ VIEW_COPY.noFlowYet }}</span>
  </span>
</template>
