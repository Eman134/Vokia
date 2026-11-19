<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import { placeableAgents } from '../team/agents'
import type { Agent } from '../team/events'
import type { AgentProfile } from '../team/teamApi'
import { useRoster } from '../team/useRoster'
import AgentPanel from './AgentPanel.vue'
import BatchProgressList from './BatchProgressList.vue'
import FlowDiagram from './FlowDiagram.vue'
import FlowNodePanel from './FlowNodePanel.vue'
import AgentActivityPanel from '../teammap/AgentActivityPanel.vue'
import { endLineKey, lineKey, START_LINE_KEY, type FlowEdge, type LineKind } from './flowGraph'
import { configChanges, toTeamFlowDraft, validationInputOf } from './flowSync'
import { problemCountLabel, validateFlow } from './flowValidation'
import { VIEW_COPY } from './orchestrationLabels'
import type { FlowProblem, OrchestrationSnapshot } from './orchestrationTypes'
import { useFeed } from '../team/useFeed'
import { useFlowDraft, type AlignAxis, type AlignMode, type FlowNodeConfig } from './useFlowDraft'
import { useOrchestration } from './useOrchestration'

const CHIP_PORTRAIT_SIZE = 24
const DRAG_THRESHOLD_PX = 4
const DROP_ZONE_SELECTOR = '[data-flow-canvas]'
const GHOST_OFFSET_PX = 12
const GHOST_PORTRAIT_SIZE = 28
const ALIGN_ICON_SIZE = 16
const ALIGN_ACTIONS: readonly { mode: AlignMode; axis: AlignAxis; label: string; path: string }[] = [
  { mode: 'center', axis: 'vertical', label: VIEW_COPY.alignCenterVertical, path: 'M12 2v20M7 6h10v4H7zM9 14h6v4H9z' },
  { mode: 'center', axis: 'horizontal', label: VIEW_COPY.alignCenterHorizontal, path: 'M2 12h20M6 7v10h4V7zM14 9v6h4V9z' },
  { mode: 'border', axis: 'vertical', label: VIEW_COPY.alignBorderVertical, path: 'M4 2v20M8 6h10v4H8zM8 14h6v4H8z' },
  { mode: 'border', axis: 'horizontal', label: VIEW_COPY.alignBorderHorizontal, path: 'M2 4h20M6 8h4v10H6zM14 8h4v6h-4z' },
]

type Selection = { kind: 'node'; id: Agent } | { kind: 'agent'; id: Agent } | { kind: 'line'; key: string }

interface AgentDrag {
  agent: Agent
  startX: number
  startY: number
  x: number
  y: number
  movable: boolean
  dragging: boolean
}

const props = defineProps<{ teamId: string }>()

const orchestration = useOrchestration()
const roster = useRoster()
const draft = useFlowDraft(props.teamId)
const {
  agents: flowAgents,
  positions,
  endpoints,
  completed,
  curves,
  addAgent,
  moveAgents,
  moveEndpoint,
  link,
  unlink,
  removeAgent,
  setKind,
  setCurve,
  setLeader,
  alignAgents,
  configOf,
  setConfig,
  linkStart,
  unlinkStart,
  linkEnd,
  unlinkEnd,
  dirty,
} = draft

const feed = useFeed()
const flowRef = ref<InstanceType<typeof FlowDiagram> | null>(null)
const selection = ref<Selection | null>(null)
const groupSelection = ref<Agent[]>([])
const agentDrag = ref<AgentDrag | null>(null)

// O Ember é o orquestrador: fica no início e no fim do fluxo, não no elenco que se arrasta.
const elencoAgents = computed(() => placeableAgents(roster.agents))
const snapshot = computed<OrchestrationSnapshot | null>(() => orchestration.snapshot)
const layers = computed(() => snapshot.value?.layers ?? [])

// Validação local (mesmas regras V1 a V7 do backend): pinta início, fim e nós e trava o Salvar fluxo.
const problems = computed(() => validateFlow(validationInputOf(draft), (id) => roster.nameOf(id)))
const problemIds = computed(() => new Set(problems.value.flatMap((problem) => (problem.node === null ? [] : [problem.node]))))
// Recusa do backend mostra a lista dele até o desenho mudar.
const refusedProblems = ref<FlowProblem[]>([])
const shownProblems = computed(() => (refusedProblems.value.length > 0 ? refusedProblems.value : problems.value))
const problemsOpen = ref(false)
const saving = ref(false)
const canSave = computed(() => dirty.value && problems.value.length === 0 && !saving.value && snapshot.value !== null)

watch(problems, () => {
  refusedProblems.value = []
})

// A estrutura vai de uma vez ao backend; as configurações dos agentes que mudaram são regravadas em seguida.
async function saveFlow(): Promise<void> {
  const current = snapshot.value
  if (current === null || !canSave.value) return
  const flow = toTeamFlowDraft(draft, current, props.teamId)
  if (flow === null) {
    feed.feed.push({ type: 'error', message: VIEW_COPY.noLayer })
    return
  }
  saving.value = true
  try {
    const result = await orchestration.saveFlow(props.teamId, flow)
    if (!result.saved) {
      refusedProblems.value = result.problems
      return
    }
    const saved = orchestration.snapshot
    const changes = saved === null ? [] : configChanges(draft, saved, props.teamId)
    for (const change of changes) {
      if (!(await orchestration.placeAgent(change))) return
    }
    draft.markSaved()
  } finally {
    saving.value = false
  }
}

const selectedNode = computed<Agent | null>(() => (selection.value?.kind === 'node' ? selection.value.id : null))
const selectedLineKey = computed<string | null>(() => (selection.value?.kind === 'line' ? selection.value.key : null))
const selectedAgent = computed<AgentProfile | null>(() =>
  selection.value?.kind === 'agent' ? (roster.profileOf(selection.value.id) ?? null) : null,
)
// Painel só existe com algo selecionado; no celular ele se sobrepõe ao canvas em vez de empurrar a página.
const panelOpen = computed(() => selectedNode.value !== null || (selectedAgent.value !== null && snapshot.value !== null))

function placementOf(agentId: Agent) {
  return snapshot.value?.placements.find((placement) => placement.agentId === agentId)
}

function clearSelection(): void {
  selection.value = null
  groupSelection.value = []
}

function selectAgent(id: Agent): void {
  selection.value = { kind: 'agent', id }
}

function selectLine(key: string): void {
  selection.value = { kind: 'line', key }
  groupSelection.value = []
}

// Linha recém-criada já fica selecionada: a barra com síncrona ou assíncrona aparece na hora.
function onLink(from: Agent, to: Agent): void {
  link(from, to)
  selectLine(lineKey({ from, to }))
}

function onLinkStart(agent: Agent): void {
  linkStart(agent)
  selectLine(START_LINE_KEY)
}

function onLinkEnd(from: Agent): void {
  linkEnd(from)
  selectLine(endLineKey(from))
}

// Linha do início ou do fim: remover só solta a ponta, que a validação passa a marcar.
function removeLine(key: string): void {
  if (key === START_LINE_KEY) unlinkStart()
  else if (key.startsWith(endLineKey(''))) unlinkEnd(key.slice(endLineKey('').length))
  clearSelection()
}

function removeSelectedLine(): void {
  const key = selectedLineKey.value
  if (key === null) return
  const edge = completed.value.manualEdges.find((candidate) => lineKey(candidate) === key)
  if (edge) removeEdge(edge)
  else removeLine(key)
}

// Shift acrescenta ou tira o nó do grupo; sem Shift, o nó clicado vira a seleção única.
function onFlowSelect(agent: Agent, additive: boolean): void {
  selection.value = { kind: 'node', id: agent }
  if (!additive) {
    groupSelection.value = [agent]
    return
  }
  groupSelection.value = groupSelection.value.includes(agent)
    ? groupSelection.value.filter((member) => member !== agent)
    : [...groupSelection.value, agent]
}

function removeEdge(edge: FlowEdge): void {
  unlink(edge.from, edge.to)
  clearSelection()
}

// Remove o agente do fluxo; as setas ligadas a ele saem e as automáticas são recalculadas.
function removeFromFlow(agent: Agent): void {
  removeAgent(agent)
  clearSelection()
}

function setEdgeKind(edge: FlowEdge, kind: LineKind): void {
  setKind(edge.from, edge.to, kind)
}

function updateSelectedConfig(patch: Partial<FlowNodeConfig>): void {
  if (selectedNode.value !== null) setConfig(selectedNode.value, patch)
}

function makeSelectedLeader(): void {
  if (selectedNode.value !== null) setLeader(selectedNode.value)
}

// Clique sem arrasto abre o agente; arrastar até a área do fluxo adiciona o agente onde foi solto.
// Quem já está no fluxo não arrasta: continua clicável, para abrir o agente.
function startAgentDrag(agent: Agent, event: PointerEvent): void {
  agentDrag.value = {
    agent,
    startX: event.clientX,
    startY: event.clientY,
    x: event.clientX,
    y: event.clientY,
    movable: !flowAgents.value.includes(agent),
    dragging: false,
  }
  window.addEventListener('pointermove', onAgentDragMove)
  window.addEventListener('pointerup', onAgentDragEnd)
  window.addEventListener('pointercancel', stopAgentDrag)
}

function onAgentDragMove(event: PointerEvent): void {
  const current = agentDrag.value
  if (!current) return
  const moved = Math.hypot(event.clientX - current.startX, event.clientY - current.startY) > DRAG_THRESHOLD_PX
  agentDrag.value = { ...current, x: event.clientX, y: event.clientY, dragging: current.dragging || (current.movable && moved) }
}

function onAgentDragEnd(event: PointerEvent): void {
  const current = agentDrag.value
  stopAgentDrag()
  if (!current) return
  if (!current.dragging) {
    selectAgent(current.agent)
    return
  }
  const dropZone = document.elementFromPoint(event.clientX, event.clientY)?.closest(DROP_ZONE_SELECTOR)
  if (!dropZone) return
  const point = flowRef.value?.toCanvasPoint(event.clientX, event.clientY)
  if (point) addAgent(current.agent, point)
}

function stopAgentDrag(): void {
  window.removeEventListener('pointermove', onAgentDragMove)
  window.removeEventListener('pointerup', onAgentDragEnd)
  window.removeEventListener('pointercancel', stopAgentDrag)
  agentDrag.value = null
}

// Cinza: já está no fluxo, ou está sendo arrastado para o fluxo. Ao soltar ou cancelar o arrasto, volta ao normal.
function isChipGreyed(agent: Agent): boolean {
  return flowAgents.value.includes(agent) || (agentDrag.value?.dragging === true && agentDrag.value.agent === agent)
}

function organize(mode: AlignMode, axis: AlignAxis): void {
  alignAgents(mode, axis, groupSelection.value)
}

// Seleção por quadrado: os nós dentro dele viram o grupo; o nó de configuração sai.
function onMarquee(agents: Agent[]): void {
  selection.value = null
  groupSelection.value = agents
}

watch(snapshot, (next) => {
  if (next === null) clearSelection()
})

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  return target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)
}

// Delete ou Backspace tiram o nó selecionado do fluxo, exceto quando o foco está num campo de texto.
function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Delete' || event.key === 'Backspace') {
    if (isTypingTarget(event.target)) return
    if (selectedNode.value !== null) {
      event.preventDefault()
      removeFromFlow(selectedNode.value)
    } else if (selectedLineKey.value !== null) {
      event.preventDefault()
      removeSelectedLine()
    }
    return
  }
  if (event.key !== 'Escape' || (selection.value === null && groupSelection.value.length === 0)) return
  event.preventDefault()
  clearSelection()
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
  stopAgentDrag()
})
</script>

<template>
  <section
    class="relative grid min-h-0 min-w-0 flex-1 grid-rows-[auto_minmax(0,1fr)] gap-5"
    :class="panelOpen ? 'lg:grid-cols-[minmax(0,1fr)_minmax(320px,400px)]' : 'lg:grid-cols-[minmax(0,1fr)]'"
  >
    <BatchProgressList class="lg:col-span-2" />
    <div class="row-start-2 flex min-h-0 min-w-0 flex-col gap-4">
      <section class="flex flex-col gap-2">
        <h3 class="text-label font-medium text-muted">{{ VIEW_COPY.agentsTitle }}</h3>
        <UiEmpty v-if="elencoAgents.length === 0" :title="VIEW_COPY.noAgents" />
        <ul v-else class="m-0 flex list-none flex-wrap gap-2 p-0">
          <li v-for="agent in elencoAgents" :key="agent.id">
            <button
              type="button"
              class="group flex touch-none select-none items-center gap-2 rounded-control border border-line bg-surface px-2.5 py-1.5 text-left"
              :class="isChipGreyed(agent.id) ? 'cursor-default opacity-40' : 'cursor-grab'"
              :aria-label="agent.name"
              :aria-pressed="selection?.kind === 'agent' && selection.id === agent.id"
              @pointerdown="startAgentDrag(agent.id, $event)"
              @keydown.enter="selectAgent(agent.id)"
            >
              <svg
                class="shrink-0 cursor-grab text-faint group-hover:text-ink"
                width="10"
                height="16"
                viewBox="0 0 10 16"
                fill="currentColor"
                aria-hidden="true"
              >
                <circle cx="2.5" cy="3" r="1.3" />
                <circle cx="7.5" cy="3" r="1.3" />
                <circle cx="2.5" cy="8" r="1.3" />
                <circle cx="7.5" cy="8" r="1.3" />
                <circle cx="2.5" cy="13" r="1.3" />
                <circle cx="7.5" cy="13" r="1.3" />
              </svg>
              <AgentPortrait :who="agent.id" :size="CHIP_PORTRAIT_SIZE" />
              <span class="text-control font-semibold text-ink">{{ agent.name }}</span>
            </button>
          </li>
        </ul>
      </section>

      <AgentActivityPanel :agents="flowAgents" />

      <div
        v-if="groupSelection.length >= 1"
        role="group"
        :aria-label="VIEW_COPY.organize"
        class="flex flex-wrap items-center gap-1"
      >
        <UiButton
          v-for="action in ALIGN_ACTIONS"
          :key="`${action.mode}-${action.axis}`"
          variant="quiet"
          size="sm"
          :disabled="groupSelection.length < 2"
          :aria-label="action.label"
          :title="action.label"
          @press="organize(action.mode, action.axis)"
        >
          <svg
            :width="ALIGN_ICON_SIZE"
            :height="ALIGN_ICON_SIZE"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path :d="action.path" />
          </svg>
        </UiButton>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <UiButton variant="primary" size="sm" :disabled="!canSave" @press="saveFlow">
          {{ saving ? VIEW_COPY.savingFlow : VIEW_COPY.saveFlow }}
        </UiButton>
        <span v-if="dirty" class="text-small text-muted">{{ VIEW_COPY.unsaved }}</span>
        <div v-if="shownProblems.length > 0" class="relative">
          <UiButton variant="quiet" size="sm" :aria-expanded="problemsOpen" @press="problemsOpen = !problemsOpen">
            <span class="text-danger">{{ problemCountLabel(shownProblems.length) }}</span>
          </UiButton>
          <ul
            v-if="problemsOpen"
            class="absolute left-0 top-full z-20 mt-1 flex w-80 max-w-[80vw] list-none flex-col gap-1 rounded-control border border-line bg-surface p-2 text-small text-ink shadow-float"
          >
            <li v-for="(problem, index) in shownProblems" :key="`${problem.code}-${index}`">
              <span class="font-mono text-tiny text-muted">{{ problem.code }}</span> {{ problem.message }}
            </li>
          </ul>
        </div>
      </div>

      <FlowDiagram
        ref="flowRef"
        class="min-h-0 flex-1"
        :agents="flowAgents"
        :positions="positions"
        :endpoints="endpoints"
        :links="completed"
        :curves="curves"
        :selected-agent="selectedNode"
        :group-selected="groupSelection"
        :selected-line="selectedLineKey"
        :config-of="configOf"
        :problem-ids="problemIds"
        @select="onFlowSelect"
        @move-nodes="moveAgents"
        @move-endpoint="moveEndpoint"
        @marquee="onMarquee"
        @clear-selection="clearSelection"
        @link="onLink"
        @select-line="selectLine"
        @bend="setCurve"
        @set-kind="setEdgeKind"
        @unlink="removeEdge"
        @remove-line="removeLine"
        @link-start="onLinkStart"
        @link-end="onLinkEnd"
      />
    </div>

    <aside
      v-if="panelOpen"
      class="absolute inset-x-0 bottom-0 z-20 max-h-[60%] min-w-0 overflow-y-auto lg:relative lg:inset-auto lg:z-auto lg:col-start-2 lg:row-start-2 lg:max-h-full"
    >
      <FlowNodePanel
        v-if="selectedNode"
        :key="selectedNode"
        :agent="selectedNode"
        :is-leader="completed.startAgent === selectedNode"
        :config="configOf(selectedNode)"
        @close="clearSelection()"
        @update="updateSelectedConfig"
        @set-leader="makeSelectedLeader"
        @remove="removeFromFlow(selectedNode)"
      />
      <AgentPanel
        v-else-if="selectedAgent && snapshot"
        :key="selectedAgent.id"
        :agent="selectedAgent"
        :placement="placementOf(selectedAgent.id)"
        :teams="snapshot.teams"
        :layers="layers"
        @close="clearSelection()"
      />
    </aside>

    <div
      v-if="agentDrag?.dragging"
      class="pointer-events-none fixed z-50 flex items-center gap-2 rounded-control border border-line bg-surface px-2.5 py-1.5"
      :style="{ left: `${agentDrag.x + GHOST_OFFSET_PX}px`, top: `${agentDrag.y + GHOST_OFFSET_PX}px` }"
    >
      <AgentPortrait :who="agentDrag.agent" :size="GHOST_PORTRAIT_SIZE" />
      <span class="text-control font-semibold text-ink">{{ roster.nameOf(agentDrag.agent) }}</span>
    </div>
  </section>
</template>
