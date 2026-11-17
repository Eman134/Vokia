<script setup lang="ts">
import { computed } from 'vue'
import { LAYER_KIND_LABELS, VIEW_COPY } from './orchestrationLabels'
import { arrowLabel, rulePhrase } from './ruleText'
import { VOCE_NODE, type HumanGate, type OrchLayer, type OrchRule } from './orchestrationTypes'

const COLUMN_WIDTH = 180
const NODE_WIDTH = 140
const NODE_HEIGHT = 56
const NODE_RADIUS = 14
const MARGIN = 24
const ARC_BASE_HEIGHT = 36
const ARC_HEIGHT_PER_SKIPPED_COLUMN = 24
const ARC_LABEL_SPACE = 24
const ARC_LABEL_GAP = 6
const ARC_LABEL_BELOW_OFFSET = 16
const ARC_HIT_WIDTH = 14
const ARC_WIDTH = 1.5
const ARC_SELECTED_WIDTH = 2.5
const DASH_PATTERN = '6 5'
const ARROWHEAD_ID = 'orch-arrowhead'
const NODE_TITLE_OFFSET = 22
const NODE_SUBTITLE_OFFSET = 40

const props = defineProps<{
  layers: OrchLayer[]
  rules: OrchRule[]
  humanGate: HumanGate
  selectedRuleId: number | null
  selectedLayerId: string | null
}>()
const emit = defineEmits<{ selectRule: [id: number]; selectLayer: [id: string] }>()

interface Node {
  key: string
  column: number
  title: string
  subtitle: string
  selected: boolean
  isYou: boolean
}

interface Arc {
  rule: OrchRule
  d: string
  labelX: number
  labelY: number
  label: string
  phrase: string
  dashed: boolean
  selected: boolean
  height: number
  fromColumn: number
  toColumn: number
}

const sortedLayers = computed(() => [...props.layers].sort((a, b) => a.position - b.position))
const youColumn = computed(() => sortedLayers.value.length)
const columnById = computed(() => new Map(sortedLayers.value.map((layer, index) => [layer.id, index])))

function columnOf(nodeId: string): number | undefined {
  return nodeId === VOCE_NODE ? youColumn.value : columnById.value.get(nodeId)
}

const youSubtitle = computed(() =>
  props.humanGate === 'nunca' ? VIEW_COPY.autoMerge : VIEW_COPY.approvesMerge,
)

const nodes = computed<Node[]>(() => [
  ...sortedLayers.value.map((layer, column) => ({
    key: layer.id,
    column,
    title: layer.name,
    subtitle: LAYER_KIND_LABELS[layer.kind],
    selected: props.selectedLayerId === layer.id,
    isYou: false,
  })),
  {
    key: VOCE_NODE,
    column: youColumn.value,
    title: VIEW_COPY.you,
    subtitle: youSubtitle.value,
    selected: false,
    isYou: true,
  },
])

interface PlacedArc {
  rule: OrchRule
  fromColumn: number
  toColumn: number
  height: number
  right: boolean
}

const placedArcs = computed<PlacedArc[]>(() =>
  props.rules.flatMap((rule) => {
    const fromColumn = columnOf(rule.fromLayer)
    const toColumn = columnOf(rule.toLayer)
    if (fromColumn === undefined || toColumn === undefined) return []
    const skipped = Math.abs(toColumn - fromColumn) - 1
    return [
      {
        rule,
        fromColumn,
        toColumn,
        height: ARC_BASE_HEIGHT + ARC_HEIGHT_PER_SKIPPED_COLUMN * skipped,
        right: toColumn > fromColumn,
      },
    ]
  }),
)

const upSpace = computed(() => {
  const heights = placedArcs.value.filter((arc) => arc.right).map((arc) => arc.height)
  return (heights.length > 0 ? Math.max(...heights) : 0) + ARC_LABEL_SPACE
})
const downSpace = computed(() => {
  const heights = placedArcs.value.filter((arc) => !arc.right).map((arc) => arc.height)
  return (heights.length > 0 ? Math.max(...heights) : 0) + ARC_LABEL_SPACE
})

const nodeTop = computed(() => MARGIN + upSpace.value)
const nodeBottom = computed(() => nodeTop.value + NODE_HEIGHT)
const width = computed(() => (youColumn.value + 1) * COLUMN_WIDTH + 2 * MARGIN)
const height = computed(() => nodeBottom.value + downSpace.value + MARGIN)

function nodeLeft(column: number): number {
  return MARGIN + column * COLUMN_WIDTH + (COLUMN_WIDTH - NODE_WIDTH) / 2
}

function nodeCenter(column: number): number {
  return nodeLeft(column) + NODE_WIDTH / 2
}

const arcs = computed<Arc[]>(() =>
  placedArcs.value.map(({ rule, fromColumn, toColumn, height: arcHeight, right }) => {
    const x1 = nodeCenter(fromColumn)
    const x2 = nodeCenter(toColumn)
    const edgeY = right ? nodeTop.value : nodeBottom.value
    const controlY = right ? edgeY - 2 * arcHeight : edgeY + 2 * arcHeight
    const apexY = right ? edgeY - arcHeight : edgeY + arcHeight
    const labelY = right ? apexY - ARC_LABEL_GAP : apexY + ARC_LABEL_BELOW_OFFSET
    return {
      rule,
      d: `M ${x1} ${edgeY} Q ${(x1 + x2) / 2} ${controlY} ${x2} ${edgeY}`,
      labelX: (x1 + x2) / 2,
      labelY,
      label: arrowLabel(rule),
      phrase: rulePhrase(rule, sortedLayers.value),
      dashed: rule.action === 'escalar',
      selected: props.selectedRuleId === rule.id,
      height: arcHeight,
      fromColumn,
      toColumn,
    }
  }),
)

function selectNodeFromKey(node: Node, event: KeyboardEvent): void {
  if (node.isYou) return
  event.preventDefault()
  emit('selectLayer', node.key)
}

function selectArcFromKey(arc: Arc, event: KeyboardEvent): void {
  event.preventDefault()
  emit('selectRule', arc.rule.id)
}
</script>

<template>
  <div class="overflow-x-auto">
    <svg
      :width="width"
      :height="height"
      :viewBox="`0 0 ${width} ${height}`"
      role="group"
      :aria-label="VIEW_COPY.flowTitle"
      class="block"
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
          <path d="M0 0 L10 5 L0 10 z" class="fill-muted" />
        </marker>
      </defs>

      <g v-for="arc in arcs" :key="arc.rule.id">
        <g
          role="button"
          tabindex="0"
          :aria-label="arc.phrase"
          class="cursor-pointer outline-none focus-visible:opacity-80"
          @click="emit('selectRule', arc.rule.id)"
          @keydown.enter="selectArcFromKey(arc, $event)"
          @keydown.space="selectArcFromKey(arc, $event)"
        >
          <path :d="arc.d" fill="none" :stroke-width="ARC_HIT_WIDTH" class="stroke-line opacity-0" />
          <path
            :d="arc.d"
            fill="none"
            :stroke-width="arc.selected ? ARC_SELECTED_WIDTH : ARC_WIDTH"
            :stroke-dasharray="arc.dashed ? DASH_PATTERN : undefined"
            :marker-end="`url(#${ARROWHEAD_ID})`"
            :class="arc.selected ? 'stroke-accent' : 'stroke-muted'"
          />
          <text
            :x="arc.labelX"
            :y="arc.labelY"
            text-anchor="middle"
            class="text-small"
            :class="arc.selected ? 'fill-accent font-semibold' : 'fill-muted'"
          >
            {{ arc.label }}
          </text>
        </g>
      </g>

      <g
        v-for="node in nodes"
        :key="node.key"
        :role="node.isYou ? 'img' : 'button'"
        :tabindex="node.isYou ? undefined : 0"
        :class="node.isYou ? '' : 'cursor-pointer outline-none focus-visible:opacity-80'"
        @click="node.isYou ? undefined : emit('selectLayer', node.key)"
        @keydown.enter="selectNodeFromKey(node, $event)"
        @keydown.space="selectNodeFromKey(node, $event)"
      >
        <rect
          :x="nodeLeft(node.column)"
          :y="nodeTop"
          :width="NODE_WIDTH"
          :height="NODE_HEIGHT"
          :rx="NODE_RADIUS"
          class="fill-surface"
          :class="node.selected ? 'stroke-accent' : 'stroke-line'"
          stroke-width="1.5"
        />
        <text
          :x="nodeCenter(node.column)"
          :y="nodeTop + NODE_TITLE_OFFSET"
          text-anchor="middle"
          class="fill-ink text-control font-semibold"
        >
          {{ node.title }}
        </text>
        <text
          :x="nodeCenter(node.column)"
          :y="nodeTop + NODE_SUBTITLE_OFFSET"
          text-anchor="middle"
          class="fill-muted text-small"
        >
          {{ node.subtitle }}
        </text>
      </g>
    </svg>
  </div>
</template>
