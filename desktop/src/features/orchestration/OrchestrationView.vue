<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import { placeableAgents } from '../team/agents'
import type { Agent } from '../team/events'
import { useRoster } from '../team/useRoster'
import BatchProgressList from './BatchProgressList.vue'
import FlowDiagram from './FlowDiagram.vue'
import FlowGatewayPanel, { type GatewayExit } from './FlowGatewayPanel.vue'
import FlowLinePanel, { type LineEditor } from './FlowLinePanel.vue'
import FlowNodePanel from './FlowNodePanel.vue'
import PanelOverlay from './PanelOverlay.vue'
import { endLineKey, lineKey, START_LINE_KEY, type FlowEdge, type LineKind } from './flowGraph'
import { configChanges, toTeamFlowDraft, validationInputOf } from './flowSync'
import { problemCountLabel, SAME_LEVEL_TOLERANCE, validateFlow } from './flowValidation'
import { VIEW_COPY } from './orchestrationLabels'
import { GATEWAY_KIND_VALUES, type FlowProblem, type GatewayKind, type OrchestrationSnapshot } from './orchestrationTypes'
import { useFeed } from '../team/useFeed'
import type { WhenEvent } from './orchestrationTypes'
import { useFlowDraft, type AlignAxis, type AlignMode, type FlowNodeConfig } from './useFlowDraft'
import { useOrchestration } from './useOrchestration'

const CHIP_PORTRAIT_SIZE = 24
const DRAG_THRESHOLD_PX = 4
const HINT_MS = 4000
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

const GATEWAY_DRAG_PREFIX = 'gateway:'
const GATEWAY_CHIP_ICON = 'M12 3l8 9-8 9-8-9z'

type Selection =
  | { kind: 'node'; id: Agent }
  | { kind: 'line'; key: string }
  | { kind: 'gateway'; id: string }

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
  gateways,
  addGateway,
  setGatewayKind,
  removeGateway,
  setMeta,
  setDefaultExit,
  metas,
} = draft

const feed = useFeed()
const flowRef = ref<InstanceType<typeof FlowDiagram> | null>(null)
const selection = ref<Selection | null>(null)
const groupSelection = ref<Agent[]>([])
const agentDrag = ref<AgentDrag | null>(null)

// O Ember é o orquestrador: fica no início e no fim do fluxo, não no elenco que se arrasta.
const elencoAgents = computed(() => placeableAgents(roster.agents))
const snapshot = computed<OrchestrationSnapshot | null>(() => orchestration.snapshot)

// Validação local (mesmas regras V1 a V7 do backend): pinta início, fim e nós e trava o Salvar fluxo.
const problems = computed(() => validateFlow(validationInputOf(draft), (id) => roster.nameOf(id)))
// Recusa do backend mostra a lista dele até o desenho mudar.
const refusedProblems = ref<FlowProblem[]>([])
const shownProblems = computed(() => (refusedProblems.value.length > 0 ? refusedProblems.value : problems.value))
const problemsOpen = ref(false)
// Cada caixinha do fluxo com problema recebe as mensagens dele (V1 a V7, do backend ou da validação local).
const problemMessages = computed(() => {
  const byNode = new Map<string, string[]>()
  for (const problem of shownProblems.value) {
    if (problem.node !== null) byNode.set(problem.node, [...(byNode.get(problem.node) ?? []), problem.message])
  }
  return byNode
})
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
const selectedGateway = computed(() => {
  const current = selection.value
  return current?.kind === 'gateway' ? (gateways.value.find((gateway) => gateway.id === current.id) ?? null) : null
})
// Saídas do gateway selecionado: as linhas que saem dele e, se ligado, o fim; cada uma com o evento e se é a padrão.
const gatewayExits = computed<GatewayExit[]>(() => {
  const gateway = selectedGateway.value
  if (gateway === null) return []
  const exits: GatewayExit[] = []
  for (const edge of completed.value.manualEdges.filter((candidate) => candidate.from === gateway.id)) {
    const meta = metas.get(lineKey(edge))
    if (meta) exits.push({ key: lineKey(edge), label: roster.nameOf(edge.to), whenEvent: meta.whenEvent, isDefault: meta.isDefault })
  }
  const endMeta = metas.get(endLineKey(gateway.id))
  if (completed.value.endAgents.includes(gateway.id) && endMeta) {
    exits.push({ key: endLineKey(gateway.id), label: VIEW_COPY.flowEnd, whenEvent: endMeta.whenEvent, isDefault: endMeta.isDefault })
  }
  return exits
})
const hint = ref<string | null>(null)
const pairLimit = computed(() => snapshot.value?.settings.pairTurnLimit ?? 0)
// Janela da ligação clicada e janela do fim (como cada agente chega até ele).
const editingLine = ref<string | null>(null)
const editingEnd = ref(false)
const GATEWAY_ACTIONS = ['delegar', 'devolver'] as const
let hintTimer: ReturnType<typeof setTimeout> | null = null
const selectedLineKey = computed<string | null>(() => (selection.value?.kind === 'line' ? selection.value.key : null))
// Painel só existe com algo selecionado; no celular ele se sobrepõe ao canvas em vez de empurrar a página.


function clearSelection(): void {
  selection.value = null
  groupSelection.value = []
  editingLine.value = null
  editingEnd.value = false
}

function nodeLabel(id: string): string {
  const gateway = gateways.value.find((candidate) => candidate.id === id)
  if (gateway === undefined) return roster.nameOf(id)
  return gateway.kind === 'paralelo' ? VIEW_COPY.gatewayParallel : VIEW_COPY.gatewayExclusive
}

// Campos editáveis de uma ligação: o início e o fim têm ação e evento fixos; a comunicação só tem limite e instrução.
function lineEditorFor(key: string): LineEditor | null {
  const meta = metas.get(key)
  if (!meta) return null
  if (key === START_LINE_KEY) {
    const leader = completed.value.startAgent
    if (leader === null) return null
    return { key, label: `${VIEW_COPY.flowStart} → ${nodeLabel(leader)}`, meta, kind: null, showEvent: true, showLimit: false, actions: ['delegar'], removable: true }
  }
  if (key.startsWith(endLineKey(''))) {
    const node = key.slice(endLineKey('').length)
    return { key, label: `${nodeLabel(node)} → ${VIEW_COPY.flowEnd}`, meta, kind: null, showEvent: true, showLimit: false, actions: ['escalar'], removable: true }
  }
  const edge = completed.value.manualEdges.find((candidate) => lineKey(candidate) === key)
  if (!edge) return null
  const between = flowAgents.value.includes(edge.from) && flowAgents.value.includes(edge.to)
  const communication = edge.communication === true
  return {
    key,
    label: `${nodeLabel(edge.from)} → ${nodeLabel(edge.to)}`,
    meta,
    kind: between && !communication ? edge.kind : null,
    showEvent: !communication,
    showLimit: true,
    actions: communication ? [] : GATEWAY_ACTIONS,
    removable: true,
  }
}

const lineEditors = computed<LineEditor[]>(() => {
  const editor = editingLine.value === null ? null : lineEditorFor(editingLine.value)
  return editor === null ? [] : [editor]
})
const endEditors = computed<LineEditor[]>(() =>
  completed.value.endAgents.map((node) => lineEditorFor(endLineKey(node))).filter((editor): editor is LineEditor => editor !== null),
)

// Clicar numa ligação existente abre a janela dela; criar uma ligação só a deixa selecionada.
function openLine(key: string): void {
  selectLine(key)
  editingLine.value = key
}

function onSelectEndpoint(which: 'start' | 'end'): void {
  clearSelection()
  if (which === 'end') {
    editingEnd.value = true
  } else if (completed.value.startAgent !== null) {
    selection.value = { kind: 'line', key: START_LINE_KEY }
    editingLine.value = START_LINE_KEY
  }
}

function setLineKind(key: string, kind: LineKind): void {
  const edge = completed.value.manualEdges.find((candidate) => lineKey(candidate) === key)
  if (edge) setKind(edge.from, edge.to, kind)
}

function removeLineByKey(key: string): void {
  const edge = completed.value.manualEdges.find((candidate) => lineKey(candidate) === key)
  if (edge) removeEdge(edge)
  else removeLine(key)
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

// Comunicação é só entre agentes da mesma linha (altura parecida); em linhas diferentes a ligação é cancelada com a dica.
function onLinkCommunication(from: Agent, to: Agent, fromY: number, toY: number): void {
  const fromAgent = flowAgents.value.includes(from)
  const toAgent = flowAgents.value.includes(to)
  const sameRow = fromAgent && toAgent && Math.abs(fromY - toY) <= SAME_LEVEL_TOLERANCE
  if (!sameRow) {
    hint.value = VIEW_COPY.communicationHint
    if (hintTimer !== null) clearTimeout(hintTimer)
    hintTimer = setTimeout(() => {
      hint.value = null
      hintTimer = null
    }, HINT_MS)
    return
  }
  link(from, to, 'sync', true)
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
  if (gateways.value.some((gateway) => gateway.id === agent)) {
    selection.value = { kind: 'gateway', id: agent }
    groupSelection.value = []
    return
  }
  selection.value = { kind: 'node', id: agent }
  if (!additive) {
    groupSelection.value = [agent]
    return
  }
  groupSelection.value = groupSelection.value.includes(agent)
    ? groupSelection.value.filter((member) => member !== agent)
    : [...groupSelection.value, agent]
}

function setExitEvent(key: string, event: WhenEvent): void {
  setMeta(key, { whenEvent: event })
}

function removeSelectedGateway(): void {
  const gateway = selectedGateway.value
  if (gateway === null) return
  removeGateway(gateway.id)
  clearSelection()
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
// Os gateways se arrastam da lista, como os agentes: o id do arraste leva o tipo.
function startGatewayDrag(kind: GatewayKind, event: PointerEvent): void {
  agentDrag.value = {
    agent: `${GATEWAY_DRAG_PREFIX}${kind}`,
    startX: event.clientX,
    startY: event.clientY,
    x: event.clientX,
    y: event.clientY,
    movable: true,
    dragging: false,
  }
  window.addEventListener('pointermove', onAgentDragMove)
  window.addEventListener('pointerup', onAgentDragEnd)
  window.addEventListener('pointercancel', stopAgentDrag)
}

function gatewayKindOfDrag(id: string): GatewayKind | null {
  return id.startsWith(GATEWAY_DRAG_PREFIX) ? (id.slice(GATEWAY_DRAG_PREFIX.length) as GatewayKind) : null
}

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
  const gatewayKind = gatewayKindOfDrag(current.agent)
  if (!current.dragging) return
  const dropZone = document.elementFromPoint(event.clientX, event.clientY)?.closest(DROP_ZONE_SELECTOR)
  if (!dropZone) return
  const point = flowRef.value?.toCanvasPoint(event.clientX, event.clientY)
  if (!point) return
  if (gatewayKind !== null) addGateway(gatewayKind, point)
  else addAgent(current.agent, point)
}

function stopAgentDrag(): void {
  window.removeEventListener('pointermove', onAgentDragMove)
  window.removeEventListener('pointerup', onAgentDragEnd)
  window.removeEventListener('pointercancel', stopAgentDrag)
  agentDrag.value = null
}

// Cinza: já está no fluxo, ou está sendo arrastado para o fluxo. Ao soltar ou cancelar o arrasto, volta ao normal.
function dragLabel(id: string): string {
  const kind = gatewayKindOfDrag(id)
  if (kind === null) return roster.nameOf(id)
  return kind === 'paralelo' ? VIEW_COPY.gatewayParallel : VIEW_COPY.gatewayExclusive
}

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
    if (selectedGateway.value !== null) {
      event.preventDefault()
      removeSelectedGateway()
    } else if (selectedNode.value !== null) {
      event.preventDefault()
      removeFromFlow(selectedNode.value)
    } else if (selectedLineKey.value !== null) {
      event.preventDefault()
      removeSelectedLine()
    }
    return
  }
  if (event.key !== 'Escape' || (selection.value === null && groupSelection.value.length === 0 && !editingEnd.value)) return
  event.preventDefault()
  clearSelection()
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => {
  if (hintTimer !== null) clearTimeout(hintTimer)
  window.removeEventListener('keydown', onKeydown)
  stopAgentDrag()
})
</script>

<template>
  <section class="relative grid min-h-0 min-w-0 flex-1 grid-cols-[minmax(0,1fr)] grid-rows-[auto_minmax(0,1fr)] gap-5">
    <BatchProgressList />
    <div class="row-start-2 flex min-h-0 min-w-0 flex-col gap-4">
      <section class="flex flex-col gap-2">
        <h3 class="text-label font-medium text-muted">{{ VIEW_COPY.agentsTitle }}</h3>
        <UiEmpty v-if="elencoAgents.length === 0" :title="VIEW_COPY.noAgents" />
        <ul v-else class="m-0 flex list-none flex-wrap gap-2 p-0">
          <li v-for="agent in elencoAgents" :key="agent.id">
            <button
              type="button"
              class="group flex touch-none select-none items-center gap-2 rounded-md border border-line bg-surface px-2.5 py-1.5 text-left"
              :class="isChipGreyed(agent.id) ? 'cursor-default opacity-40' : 'cursor-grab'"
              :aria-label="agent.name"
              @pointerdown="startAgentDrag(agent.id, $event)"
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
          <li v-for="kind in GATEWAY_KIND_VALUES" :key="kind">
            <button
              type="button"
              class="group flex cursor-grab touch-none select-none items-center gap-2 rounded-md border border-line bg-surface px-2.5 py-1.5 text-left"
              :aria-label="kind === 'paralelo' ? VIEW_COPY.gatewayParallel : VIEW_COPY.gatewayExclusive"
              @pointerdown="startGatewayDrag(kind, $event)"
            >
              <svg class="shrink-0 text-faint group-hover:text-ink" width="10" height="16" viewBox="0 0 10 16" fill="currentColor" aria-hidden="true">
                <circle cx="2.5" cy="3" r="1.3" />
                <circle cx="7.5" cy="3" r="1.3" />
                <circle cx="2.5" cy="8" r="1.3" />
                <circle cx="7.5" cy="8" r="1.3" />
                <circle cx="2.5" cy="13" r="1.3" />
                <circle cx="7.5" cy="13" r="1.3" />
              </svg>
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" class="text-ink" aria-hidden="true">
                <path :d="GATEWAY_CHIP_ICON" />
                <path v-if="kind === 'paralelo'" d="M12 8v8M8 12h8" stroke-linecap="round" />
                <path v-else d="M9 9l6 6M15 9l-6 6" stroke-linecap="round" />
              </svg>
              <span class="text-control font-semibold text-ink">
                {{ kind === 'paralelo' ? VIEW_COPY.gatewayKindParallel : VIEW_COPY.gatewayKindExclusive }}
              </span>
            </button>
          </li>
        </ul>
      </section>

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
        <span v-if="hint" class="text-small text-danger" role="status">{{ hint }}</span>
        <div v-if="shownProblems.length > 0" class="relative">
          <UiButton variant="quiet" size="sm" :aria-expanded="problemsOpen" @press="problemsOpen = !problemsOpen">
            <span class="text-danger">{{ problemCountLabel(shownProblems.length) }}</span>
          </UiButton>
          <ul
            v-if="problemsOpen"
            class="absolute left-0 top-full z-20 mt-1 flex w-80 max-w-[80vw] list-none flex-col gap-1 rounded-md border border-line bg-surface p-2 text-small text-ink shadow-paper"
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
        :gateways="gateways"
        :positions="positions"
        :endpoints="endpoints"
        :links="completed"
        :curves="curves"
        :selected-agent="selectedNode"
        :group-selected="groupSelection"
        :selected-line="selectedLineKey"
        :config-of="configOf"
        :problems="problemMessages"
        @select="onFlowSelect"
        @move-nodes="moveAgents"
        @move-endpoint="moveEndpoint"
        @marquee="onMarquee"
        @clear-selection="clearSelection"
        @link="onLink"
        @link-communication="onLinkCommunication"
        @select-line="openLine"
        @select-endpoint="onSelectEndpoint"
        @bend="setCurve"
        @set-kind="setEdgeKind"
        @unlink="removeEdge"
        @remove-line="removeLine"
        @link-start="onLinkStart"
        @link-end="onLinkEnd"
      />
    </div>

    <PanelOverlay v-if="lineEditors.length > 0" @close="clearSelection()">
      <FlowLinePanel
        :title="VIEW_COPY.lineTitle"
        :lines="lineEditors"
        :pair-limit="pairLimit"
        @close="clearSelection()"
        @update="setMeta"
        @set-kind="setLineKind"
        @remove="removeLineByKey"
      />
    </PanelOverlay>
    <PanelOverlay v-else-if="editingEnd" @close="clearSelection()">
      <FlowLinePanel
        :title="VIEW_COPY.endTitle"
        :lines="endEditors"
        :pair-limit="pairLimit"
        @close="clearSelection()"
        @update="setMeta"
        @set-kind="setLineKind"
        @remove="removeLineByKey"
      />
    </PanelOverlay>
    <PanelOverlay v-else-if="selectedNode" @close="clearSelection()">
      <FlowNodePanel
        :key="selectedNode"
        :agent="selectedNode"
        :is-leader="completed.startAgent === selectedNode"
        :config="configOf(selectedNode)"
        @close="clearSelection()"
        @update="updateSelectedConfig"
        @set-leader="makeSelectedLeader"
        @remove="removeFromFlow(selectedNode)"
      />
    </PanelOverlay>
    <PanelOverlay v-else-if="selectedGateway" @close="clearSelection()">
      <FlowGatewayPanel
        :key="selectedGateway.id"
        :kind="selectedGateway.kind"
        :exits="gatewayExits"
        @close="clearSelection()"
        @set-kind="setGatewayKind(selectedGateway.id, $event)"
        @set-event="setExitEvent"
        @set-default="setDefaultExit(selectedGateway.id, $event)"
        @remove="removeSelectedGateway"
      />
    </PanelOverlay>

    <div
      v-if="agentDrag?.dragging"
      class="pointer-events-none fixed z-50 flex items-center gap-2 rounded-md border border-line bg-surface px-2.5 py-1.5"
      :style="{ left: `${agentDrag.x + GHOST_OFFSET_PX}px`, top: `${agentDrag.y + GHOST_OFFSET_PX}px` }"
    >
      <svg v-if="gatewayKindOfDrag(agentDrag.agent)" width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">
        <path :d="GATEWAY_CHIP_ICON" />
      </svg>
      <AgentPortrait v-else :who="agentDrag.agent" :size="GHOST_PORTRAIT_SIZE" />
      <span class="text-control font-semibold text-ink">{{ dragLabel(agentDrag.agent) }}</span>
    </div>
  </section>
</template>
