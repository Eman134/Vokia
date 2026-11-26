<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import { useProjects } from '../projects/useProjects'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import {
  getContextGraph,
  type ContextGraph,
  type ContextGraphEdge,
  type ContextGraphNode,
  type ContextGraphOptimization,
  type ContextGraphOptimizationKind,
} from './learningApi'
import { GRAPH_DESCRIPTION, GRAPH_EMPTY_TITLE } from './memoriaSample'

interface PositionedNode {
  node: ContextGraphNode
  x: number
  y: number
}

interface PositionedEdge {
  edge: ContextGraphEdge
  d: string
}

const PORTRAIT_SIZE = 20
const HEADER_CLASSES = 'flex flex-wrap items-baseline justify-between gap-2 px-4.5 py-4'
const CANVAS_CLASSES = 'relative h-[470px] w-full min-w-[620px]'
const LEGEND_CLASSES =
  'absolute left-4 top-3 z-10 flex flex-wrap gap-3.5 rounded-sm border border-line bg-surface px-2.5 py-1 text-meta text-muted'
const OPTIMIZATIONS_CLASSES = 'flex flex-col gap-3 border-t border-line px-4.5 py-4'

const BADGE_CLASSES: Record<ContextGraphOptimizationKind, string> = {
  conflict: 'bg-warn-soft text-warn-text',
  context: 'bg-accent-soft text-accent',
  artifact: 'bg-accent-soft text-accent',
  order: 'bg-surface-2 text-muted',
}

const projects = useProjects()

const graph = ref<ContextGraph | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const selectedNodeId = ref<string | null>(null)
const expandedOptId = ref<string | null>(null)

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
  } catch (err) {
    error.value = `Não foi possível montar o grafo: ${String(err)}`
    graph.value = null
  } finally {
    loading.value = false
  }
}

watch(
  () => projects.selected?.id,
  (projectId) => {
    void loadGraph(projectId)
  },
  { immediate: true },
)

function distributeX(index: number, total: number): number {
  if (total <= 1) return 50
  const margin = 12
  const span = 100 - margin * 2
  return margin + (span * index) / (total - 1)
}

const positionedNodes = computed<PositionedNode[]>(() => {
  const nodes = graph.value?.nodes ?? []
  const cards = nodes.filter((n) => n.kind === 'card')
  const files = nodes.filter((n) => n.kind === 'file')
  const artifacts = nodes.filter((n) => n.kind === 'artifact')

  const result: PositionedNode[] = []

  cards.forEach((node, idx) => {
    result.push({
      node,
      x: distributeX(idx, cards.length),
      y: 20,
    })
  })

  files.forEach((node, idx) => {
    const stagger = files.length > 3 ? (idx % 2 === 0 ? -5 : 6) : 0
    result.push({
      node,
      x: distributeX(idx, files.length),
      y: (cards.length > 0 ? 52 : 38) + stagger,
    })
  })

  artifacts.forEach((node, idx) => {
    result.push({
      node,
      x: distributeX(idx, artifacts.length),
      y: 84,
    })
  })

  return result
})

const nodePositionMap = computed(() => {
  const map = new Map<string, PositionedNode>()
  for (const item of positionedNodes.value) {
    map.set(item.node.id, item)
  }
  return map
})

const positionedEdges = computed<PositionedEdge[]>(() => {
  const edges = graph.value?.edges ?? []
  const posMap = nodePositionMap.value
  const list: PositionedEdge[] = []
  for (const edge of edges) {
    const src = posMap.get(edge.source)
    const dst = posMap.get(edge.target)
    if (!src || !dst) continue
    const midY = (src.y + dst.y) / 2
    const d = `M${src.x} ${src.y} C${src.x} ${midY}, ${dst.x} ${midY}, ${dst.x} ${dst.y}`
    list.push({ edge, d })
  }
  return list
})

const highlightedIds = computed<Set<string>>(() => {
  const ids = new Set<string>()
  const active = selectedNodeId.value
  if (!active) return ids
  ids.add(active)
  for (const edge of graph.value?.edges ?? []) {
    if (edge.source === active) ids.add(edge.target)
    if (edge.target === active) ids.add(edge.source)
  }
  return ids
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
  return 'var(--color-accent)'
}

function edgeDash(kind: ContextGraphEdge['kind']): string | undefined {
  if (kind === 'conflict_risk') return '4 3'
  if (kind === 'artifact_link') return '2 4'
  return undefined
}

function edgeOpacity(edge: ContextGraphEdge): number {
  if (!selectedNodeId.value) return 0.9
  return edge.source === selectedNodeId.value || edge.target === selectedNodeId.value ? 1 : 0.2
}

function nodeOpacity(id: string): string {
  if (!selectedNodeId.value) return 'opacity-100'
  return highlightedIds.value.has(id) ? 'opacity-100' : 'opacity-40'
}

function fileNodeClasses(node: ContextGraphNode): string {
  const base =
    'absolute -translate-x-1/2 -translate-y-1/2 flex items-center gap-1.5 rounded-md px-2.5 py-1.5 font-mono text-small transition-opacity cursor-pointer'
  if (node.conflictRisk) {
    return `${base} border-[1.5px] border-dashed border-warn bg-warn-soft text-ink`
  }
  if (node.cardIds.length > 0) {
    return `${base} border border-accent bg-accent-soft text-ink`
  }
  return `${base} border border-line bg-surface text-ink`
}

function cardNodeClasses(): string {
  return 'absolute -translate-x-1/2 -translate-y-1/2 flex items-center gap-2 rounded-md border border-accent bg-surface px-2.5 py-1.5 text-small text-ink transition-opacity cursor-pointer'
}

function artifactNodeClasses(): string {
  return 'absolute -translate-x-1/2 -translate-y-1/2 flex items-center gap-2 rounded-md border border-accent bg-accent-soft px-2.5 py-1.5 text-small text-ink whitespace-nowrap transition-opacity cursor-pointer'
}
</script>

<template>
  <div :class="HEADER_CLASSES">
    <div class="flex flex-wrap items-baseline gap-2">
      <h2 class="text-lead text-ink">Grafo</h2>
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
    text="Quando cards ativos, passos de trabalho ou artefatos existirem no projeto, o grafo aparece aqui."
  />

  <template v-else-if="hasNodes">
    <div class="overflow-x-auto">
      <div
        :class="CANVAS_CLASSES"
        style="
          background-image: radial-gradient(var(--color-line) 1.2px, transparent 1.2px);
          background-size: 18px 18px;
        "
      >
        <div :class="LEGEND_CLASSES">
          <span class="inline-flex items-center gap-1.5">
            <span class="inline-block h-0.5 w-4 bg-accent align-middle" />
            caminho da tarefa
          </span>
          <span class="inline-flex items-center gap-1.5">
            <span class="inline-block w-4 border-t-[1.5px] border-dashed border-warn align-middle" />
            risco de conflito
          </span>
          <span class="inline-flex items-center gap-1.5">
            <span class="inline-block w-4 border-t-[1.5px] border-dotted border-accent align-middle" />
            artefato descreve
          </span>
        </div>

        <svg
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
          aria-hidden="true"
          class="pointer-events-none absolute inset-0 h-full w-full"
        >
          <path
            v-for="item in positionedEdges"
            :key="item.edge.id"
            :d="item.d"
            :stroke="edgeStroke(item.edge.kind)"
            :stroke-width="item.edge.kind === 'task_path' ? 2 : 1.5"
            :stroke-dasharray="edgeDash(item.edge.kind)"
            :stroke-opacity="edgeOpacity(item.edge)"
            vector-effect="non-scaling-stroke"
            fill="none"
          />
        </svg>

        <template v-for="pos in positionedNodes" :key="pos.node.id">
          <button
            v-if="pos.node.kind === 'card'"
            type="button"
            :class="[cardNodeClasses(), nodeOpacity(pos.node.id)]"
            :style="{ left: `${pos.x}%`, top: `${pos.y}%` }"
            :title="pos.node.subtitle ?? pos.node.label"
            @click="toggleNode(pos.node.id)"
          >
            <AgentPortrait
              v-if="pos.node.agents.length > 0"
              :who="pos.node.agents[0]"
              :size="PORTRAIT_SIZE"
            />
            <span class="flex flex-col items-start text-left leading-tight">
              <span class="text-tiny font-semibold text-accent">{{ pos.node.label }}</span>
              <span v-if="pos.node.subtitle" class="max-w-[140px] truncate text-meta text-ink">
                {{ pos.node.subtitle }}
              </span>
            </span>
          </button>

          <button
            v-else-if="pos.node.kind === 'file'"
            type="button"
            :class="[fileNodeClasses(pos.node), nodeOpacity(pos.node.id)]"
            :style="{ left: `${pos.x}%`, top: `${pos.y}%` }"
            :title="pos.node.path ?? pos.node.label"
            @click="toggleNode(pos.node.id)"
          >
            <AgentPortrait
              v-for="agent in pos.node.agents.slice(0, 2)"
              :key="agent"
              :who="agent"
              :size="PORTRAIT_SIZE"
            />
            <span>{{ pos.node.label }}</span>
            <span
              v-if="pos.node.conflictRisk"
              class="rounded-sm bg-warn px-1 py-0.5 font-sans text-tiny font-semibold text-accent-ink"
            >
              conflito
            </span>
          </button>

          <button
            v-else
            type="button"
            :class="[artifactNodeClasses(), nodeOpacity(pos.node.id)]"
            :style="{ left: `${pos.x}%`, top: `${pos.y}%` }"
            :title="pos.node.subtitle ?? pos.node.label"
            @click="toggleNode(pos.node.id)"
          >
            <svg width="13" height="13" viewBox="0 0 16 16" aria-hidden="true" class="flex-none text-accent">
              <path
                d="M4 1.5h5.5l3 3v10H4zM9.5 1.5v3h3M6 8h4.5M6 10.5h4.5"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
            <span class="flex flex-col items-start text-left leading-tight">
              <span class="text-tiny text-muted">{{ pos.node.artifactKind ?? 'artefato' }}</span>
              <span class="max-w-[160px] truncate font-semibold text-ink">{{ pos.node.label }}</span>
            </span>
          </button>
        </template>
      </div>
    </div>

    <div :class="OPTIMIZATIONS_CLASSES">
      <h3 class="text-label font-semibold text-ink">Otimizações sugeridas</h3>
      <p v-if="optimizations.length === 0" class="text-small text-muted">
        Nenhuma otimização pendente para o estado atual do grafo.
      </p>
      <div v-for="opt in optimizations" :key="opt.id" class="flex flex-col gap-1.5">
        <div class="flex items-center gap-2.5 text-control">
          <span
            class="flex-none rounded-sm px-2 py-0.5 text-meta font-medium"
            :class="BADGE_CLASSES[opt.kind]"
          >
            {{ opt.badge }}
          </span>
          <span class="min-w-0 flex-1 text-ink">{{ opt.message }}</span>
          <UiButton
            v-if="opt.files.length > 0 || opt.cardIds.length > 0"
            variant="quiet"
            size="sm"
            @press="toggleOptFiles(opt)"
          >
            {{ expandedOptId === opt.id ? 'Ocultar' : 'Ver arquivos' }}
          </UiButton>
        </div>
        <div
          v-if="expandedOptId === opt.id && opt.files.length > 0"
          class="flex flex-wrap gap-1.5 pl-16"
        >
          <span
            v-for="file in opt.files"
            :key="file"
            class="rounded-sm bg-surface-2 px-2 py-0.5 font-mono text-tiny text-muted"
          >
            {{ file }}
          </span>
        </div>
      </div>
    </div>
  </template>
</template>
