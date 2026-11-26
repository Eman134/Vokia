<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import MarkdownText from '../team/MarkdownText.vue'
import UiButton from '../../ui/UiButton.vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import {
  DEFAULT_JITTER,
  sketchDiamond,
  sketchEllipse,
  sketchQuadratic,
  sketchRect,
} from '../../ui/sketch/sketchPath'
import { useLearningSuggestions } from './useLearningSuggestions'
import {
  getContextGraph,
  memoryGraphQuery,
  memoryGraphSave,
  scanProjectGraph,
  type ContextGraphEdge,
  type ContextGraphNode,
  type ContextGraphOptimization,
  type ProjectMemory,
} from './learningApi'

const props = defineProps<{
  open: boolean
  projectId: number | null | undefined
  title: string
  query: string
  contextText: string
  cardId?: string
  agent?: string
  metaSummary?: string
}>()

const emit = defineEmits<{
  close: []
}>()

const learning = useLearningSuggestions()

const loading = ref(false)
const error = ref<string | null>(null)
const nodes = ref<ContextGraphNode[]>([])
const edges = ref<ContextGraphEdge[]>([])
const memories = ref<ProjectMemory[]>([])
const optimizations = ref<ContextGraphOptimization[]>([])
const selectedNodeId = ref<string | null>(null)

const draftFact = ref('')
const draftCategory = ref<'decisao' | 'macro' | 'mapa-arquivos'>('decisao')
const draftKeywords = ref('')
const saving = ref(false)
const scanningRepo = ref(false)
const saveFeedback = ref<string | null>(null)

async function runRepoScan(): Promise<void> {
  if (!props.projectId || scanningRepo.value) return
  scanningRepo.value = true
  error.value = null
  saveFeedback.value = null
  try {
    const outcome = await scanProjectGraph(props.projectId)
    saveFeedback.value = `Varredura concluída por ${outcome.agent}: ${outcome.createdMemories} memórias e ${outcome.linkedFiles} arquivos conectados.`
    await learning.load(props.projectId)
    await load()
  } catch (err) {
    error.value = `Falha na varredura do repositório: ${String(err)}`
  } finally {
    scanningRepo.value = false
  }
}

const NODE_COLOR: Record<ContextGraphNode['kind'], string> = {
  decision: 'var(--color-mark-pink)',
  keyword: 'var(--color-mark-blue)',
  memory: 'var(--color-mark-green)',
  card: 'var(--color-mark-yellow)',
  file: 'var(--color-accent-soft)',
  artifact: 'var(--color-warn-soft)',
}

function hashSeed(id: string): number {
  let h = 2166136261
  for (let i = 0; i < id.length; i += 1) {
    h ^= id.charCodeAt(i)
    h = Math.imul(h, 16777619)
  }
  return h >>> 0
}

function modalNodeSketchPath(pos: { node: ContextGraphNode; radius: number }): string {
  const d = pos.radius * 2
  const seed = hashSeed(pos.node.id)
  if (pos.node.kind === 'decision') return sketchDiamond(d + 4, d + 4, seed, DEFAULT_JITTER * 0.85)
  if (pos.node.kind === 'artifact') return sketchRect(d + 2, d + 2, seed, DEFAULT_JITTER * 0.85)
  return sketchEllipse(d + 3, d + 3, seed, DEFAULT_JITTER * 0.85)
}

function modalNodeOffset(pos: { node: ContextGraphNode; radius: number }): number {
  if (pos.node.kind === 'decision') return pos.radius + 2
  if (pos.node.kind === 'artifact') return pos.radius + 1
  return pos.radius + 1.5
}

const positionedNodes = computed(() => {
  const all = nodes.value
  if (all.length === 0) return []
  const deg = new Map<string, number>()
  for (const e of edges.value) {
    deg.set(e.source, (deg.get(e.source) ?? 0) + 1)
    deg.set(e.target, (deg.get(e.target) ?? 0) + 1)
  }
  const W = 680
  const H = 280
  const cx = W / 2
  const cy = H / 2
  const sim = all.map((node, i) => {
    const angle = (2 * Math.PI * i) / Math.max(1, all.length)
    const r = node.kind === 'card' ? 60 : 105
    const degree = deg.get(node.id) ?? 0
    return {
      node,
      x: cx + Math.cos(angle) * r,
      y: cy + Math.sin(angle) * r,
      vx: 0,
      vy: 0,
      radius: Math.min(15, 6 + Math.sqrt(degree) * 3),
    }
  })
  const byId = new Map(sim.map((s, idx) => [s.node.id, idx]))
  for (let step = 0; step < 80; step++) {
    const cool = 1 - step / 90
    for (let i = 0; i < sim.length; i++) {
      for (let j = i + 1; j < sim.length; j++) {
        const a = sim[i]
        const b = sim[j]
        const dx = b.x - a.x || 1
        const dy = b.y - a.y || 1
        const d2 = Math.max(4, dx * dx + dy * dy)
        const f = (2400 / d2) * cool
        const d = Math.sqrt(d2)
        a.vx -= (dx / d) * f
        a.vy -= (dy / d) * f
        b.vx += (dx / d) * f
        b.vy += (dy / d) * f
      }
    }
    for (const e of edges.value) {
      const ia = byId.get(e.source)
      const ib = byId.get(e.target)
      if (ia === undefined || ib === undefined) continue
      const a = sim[ia]
      const b = sim[ib]
      const dx = b.x - a.x
      const dy = b.y - a.y
      const d = Math.max(1, Math.sqrt(dx * dx + dy * dy))
      const f = (d - 95) * 0.06 * cool
      a.vx += (dx / d) * f
      a.vy += (dy / d) * f
      b.vx -= (dx / d) * f
      b.vy -= (dy / d) * f
    }
    for (const s of sim) {
      s.vx += (cx - s.x) * 0.02 * cool
      s.vy += (cy - s.y) * 0.02 * cool
      s.x = Math.max(46, Math.min(W - 46, s.x + s.vx * 0.65))
      s.y = Math.max(32, Math.min(H - 36, s.y + s.vy * 0.65))
      s.vx *= 0.55
      s.vy *= 0.55
    }
  }
  return sim
})

const positionedEdges = computed(() => {
  const map = new Map(positionedNodes.value.map((p) => [p.node.id, p]))
  const list: Array<{ edge: ContextGraphEdge; d: string }> = []
  for (const edge of edges.value) {
    const src = map.get(edge.source)
    const dst = map.get(edge.target)
    if (!src || !dst) continue
    const dx = dst.x - src.x
    const dy = dst.y - src.y
    const cx = (src.x + dst.x) / 2 - dy * 0.12
    const cy = (src.y + dst.y) / 2 + dx * 0.12
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

const selectedNode = computed(() => nodes.value.find((n) => n.id === selectedNodeId.value) ?? null)

async function load(): Promise<void> {
  if (!props.open || props.projectId == null) return
  loading.value = true
  error.value = null
  saveFeedback.value = null
  selectedNodeId.value = null
  const cleanText = props.contextText.replace(/\s+/g, ' ').trim()
  draftFact.value = cleanText.length > 220 ? `${cleanText.slice(0, 217)}…` : cleanText
  try {
    const q = props.query.trim()
    const res = await memoryGraphQuery(props.projectId, q, 20)
    if (res.nodes.length === 0 && q !== '') {
      const full = await getContextGraph(props.projectId)
      nodes.value = full.nodes
      edges.value = full.edges
      optimizations.value = full.optimizations
    } else {
      nodes.value = res.nodes
      edges.value = res.edges
      optimizations.value = res.optimizations
    }
    memories.value = res.memories
  } catch (err) {
    error.value = `Erro ao carregar grafo: ${String(err)}`
  } finally {
    loading.value = false
  }
}

async function saveToGraph(): Promise<void> {
  if (props.projectId == null || !draftFact.value.trim()) return
  saving.value = true
  saveFeedback.value = null
  try {
    const keywords = draftKeywords.value
      .split(',')
      .map((k) => k.trim().toLowerCase())
      .filter((k) => k.length >= 3)
    await memoryGraphSave({
      projectId: props.projectId,
      fact: draftFact.value.trim(),
      category: draftCategory.value,
      keywords,
      cardId: props.cardId,
      author: props.agent ?? 'você',
    })
    saveFeedback.value = 'Salvo no grafo de memória para otimizações futuras.'
    draftKeywords.value = ''
    await learning.load(props.projectId)
    await load()
  } catch (err) {
    saveFeedback.value = `Não foi possível salvar: ${String(err)}`
  } finally {
    saving.value = false
  }
}

function onKeyDown(e: KeyboardEvent): void {
  if (props.open && e.key === 'Escape') {
    emit('close')
  }
}

watch(
  () => [props.open, props.projectId, props.query],
  () => {
    if (props.open) void load()
  },
  { immediate: true },
)

onMounted(() => window.addEventListener('keydown', onKeyDown))
onUnmounted(() => window.removeEventListener('keydown', onKeyDown))
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
      role="dialog"
      aria-modal="true"
      @click.self="emit('close')"
    >
      <section class="flex max-h-[88vh] w-full max-w-4xl flex-col overflow-hidden rounded-lg border border-line bg-surface shadow-lg">
        <header class="flex items-center justify-between gap-3 border-b border-line px-5 py-3.5">
          <div class="min-w-0">
            <h2 class="truncate text-section text-ink">{{ title }}</h2>
            <p v-if="metaSummary" class="truncate text-meta text-muted">{{ metaSummary }}</p>
          </div>
          <UiCloseButton @press="emit('close')" />
        </header>

        <div class="flex flex-1 flex-col gap-4 overflow-y-auto p-5">
          <p v-if="error" class="text-caption text-danger">{{ error }}</p>

          <div class="rounded-md border border-line bg-surface-2">
            <div class="flex items-center justify-between gap-2 border-b border-line px-3.5 py-2 text-meta text-muted">
              <span>Grafo relacionado ({{ nodes.length }} nós · {{ edges.length }} conexões)</span>
              <div class="flex items-center gap-2">
                <span v-if="loading">Carregando...</span>
                <UiButton
                  v-if="projectId"
                  variant="quiet"
                  size="sm"
                  :disabled="scanningRepo"
                  data-testid="modal-scan-repo"
                  @press="runRepoScan"
                >
                  {{ scanningRepo ? 'Mapeando repositório...' : 'Mapear repositório' }}
                </UiButton>
              </div>
            </div>
            <div v-if="nodes.length === 0 && !loading" class="p-4 text-caption text-muted">
              Nenhum nó conectado ainda para este contexto.
            </div>
            <div
              v-else
              class="relative h-[280px] w-full overflow-hidden"
              style="background-color: light-dark(#fcf6e3, #17171c);"
            >
              <svg viewBox="0 0 680 280" preserveAspectRatio="xMidYMid meet" class="h-full w-full">
                <path
                  v-for="item in positionedEdges"
                  :key="item.edge.id"
                  :d="item.d"
                  :stroke="item.edge.kind === 'conflict_risk' ? 'var(--color-warn)' : item.edge.kind === 'touches_file' || item.edge.kind === 'about_card' ? 'var(--color-ok)' : item.edge.kind === 'has_keyword' ? 'var(--color-accent)' : 'var(--color-graphite)'"
                  :stroke-opacity="selectedNodeId ? (item.edge.source === selectedNodeId || item.edge.target === selectedNodeId ? 1 : 0.16) : 0.65"
                  stroke-width="1.4"
                  stroke-linecap="round"
                  fill="none"
                />
                <g
                  v-for="pos in positionedNodes"
                  :key="pos.node.id"
                  class="cursor-pointer"
                  @click="selectedNodeId = selectedNodeId === pos.node.id ? null : pos.node.id"
                >
                  <g :transform="`translate(${pos.x - modalNodeOffset(pos) + 1.5}, ${pos.y - modalNodeOffset(pos) + 2})`">
                    <path :d="modalNodeSketchPath(pos)" :fill="NODE_COLOR[pos.node.kind]" fill-opacity="0.92" />
                  </g>
                  <g :transform="`translate(${pos.x - modalNodeOffset(pos)}, ${pos.y - modalNodeOffset(pos)})`">
                    <path
                      :d="modalNodeSketchPath(pos)"
                      fill="none"
                      :stroke="selectedNodeId === pos.node.id ? 'var(--color-accent)' : 'var(--color-ink)'"
                      :stroke-width="selectedNodeId === pos.node.id ? 2 : 1.4"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                  </g>
                  <text
                    :x="pos.x"
                    :y="pos.y + pos.radius + 15"
                    text-anchor="middle"
                    fill="var(--color-ink)"
                    font-size="10.5"
                    font-weight="600"
                    class="pointer-events-none select-none"
                    style="paint-order: stroke; stroke: light-dark(#fcf6e3, #17171c); stroke-width: 4px; stroke-linejoin: round;"
                  >
                    {{ pos.node.label.length > 26 ? `${pos.node.label.slice(0, 25)}…` : pos.node.label }}
                  </text>
                </g>
              </svg>
            </div>
            <div v-if="selectedNode" class="flex items-start gap-2 border-t border-line bg-surface px-3.5 py-2 text-small text-ink">
              <span class="mt-0.5 shrink-0 rounded-sm bg-accent-soft px-1.5 py-0.5 text-tiny font-semibold text-accent">{{ selectedNode.kind }}</span>
              <MarkdownText :text="selectedNode.subtitle ?? selectedNode.path ?? selectedNode.label" class="min-w-0 flex-1" />
            </div>
          </div>

          <div class="grid gap-4 md:grid-cols-2">
            <div class="flex min-w-0 flex-col gap-1.5 rounded-md border border-line p-3.5">
              <h3 class="text-label font-semibold text-ink">Contexto completo</h3>
              <MarkdownText
                :text="contextText || 'Sem conteúdo textual.'"
                class="max-h-48 overflow-y-auto wrap-break-word text-small text-ink"
              />
            </div>

            <div class="flex flex-col gap-2 rounded-md border border-line p-3.5">
              <h3 class="text-label font-semibold text-ink">Otimizar e salvar no grafo</h3>
              <select
                v-model="draftCategory"
                class="rounded-sm border border-line bg-surface px-2 py-1 text-small text-ink"
              >
                <option value="decisao">Decisão / Regra</option>
                <option value="macro">Visão macro</option>
                <option value="mapa-arquivos">Mapa de arquivos</option>
              </select>
              <input
                v-model="draftKeywords"
                type="text"
                class="w-full rounded-sm border border-line bg-surface px-2 py-1 text-small text-ink"
                placeholder="Palavras-chave separadas por vírgula (ex: triagem, grafo, auth)"
              />
              <textarea
                v-model="draftFact"
                rows="4"
                class="w-full rounded-sm border border-line bg-surface p-2 text-small text-ink"
                placeholder="Resumo relevante para futuros turnos..."
              />
              <div class="flex items-center justify-between gap-2">
                <span v-if="saveFeedback" class="text-tiny text-muted">{{ saveFeedback }}</span>
                <UiButton
                  variant="primary"
                  size="sm"
                  class="ml-auto"
                  :disabled="saving || !draftFact.trim() || projectId == null"
                  @press="saveToGraph"
                >
                  {{ saving ? 'Salvando...' : 'Salvar no grafo' }}
                </UiButton>
              </div>
            </div>
          </div>

          <div v-if="memories.length > 0 || optimizations.length > 0" class="flex flex-col gap-1.5 rounded-md border border-line p-3.5">
            <h3 class="text-label font-semibold text-ink">Memórias e sugestões vinculadas</h3>
            <ul class="flex flex-col gap-1.5 text-small text-muted">
              <li v-for="mem in memories.slice(0, 5)" :key="mem.id" class="flex items-start gap-1.5">
                <span class="shrink-0 font-semibold">[{{ mem.wing ?? mem.source }}]</span>
                <MarkdownText :text="mem.fact" class="min-w-0 flex-1" />
              </li>
              <li v-for="opt in optimizations.slice(0, 3)" :key="opt.id" class="flex items-start gap-1.5">
                <span class="shrink-0 font-semibold">[{{ opt.badge }}]</span>
                <MarkdownText :text="opt.message" class="min-w-0 flex-1" />
              </li>
            </ul>
          </div>
        </div>
      </section>
    </div>
  </Teleport>
</template>
