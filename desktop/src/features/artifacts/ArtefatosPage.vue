<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import UiButton from '../../ui/UiButton.vue'
import UiCreateButton from '../../ui/UiCreateButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import { DOT_GRID_CLASSES } from '../../ui/dotGrid'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiNotice from '../../ui/UiNotice.vue'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import UiSelect from '../../ui/UiSelect.vue'
import { PAGE_LABELS } from '../../shell/railNav'
import { useNavigation } from '../../shell/useNavigation'
import { useBacklog } from '../backlog/useBacklog'
import { parseProposal } from '../backlog/planProposal'
import { useItemPlan } from '../backlog/useItemPlan'
import { useProjects } from '../projects/useProjects'
import { planArtifactId, type ArtifactDocument, type ArtifactSummary } from './artifactApi'
import { ALL_STATES, groupEntries, matchesQuery, matchesState, type BrowsableArtifact, type StateFilter } from './artifactBrowse'
import ArtifactCard from './ArtifactCard.vue'
import ArtifactDocumentView from './ArtifactDocumentView.vue'
import ArtifactEditorDialog from './ArtifactEditorDialog.vue'
import ArtifactListRow from './ArtifactListRow.vue'
import {
  ARTIFACTS_ALL_LABEL,
  ARTIFACTS_BACK_LABEL,
  ARTIFACTS_EMPTY_TITLE,
  ARTIFACTS_FILTER_LABEL,
  ARTIFACTS_LOADING_TEXT,
  ARTIFACTS_NEW_LABEL,
  ARTIFACTS_OPENING_TEXT,
  ARTIFACTS_PLANS_LABEL,
  CLEAR_FILTERS_LABEL,
  GROUP_LABEL,
  IMPORT_LABEL,
  kindTabLabel,
  LAYOUT_LABEL,
  LAYOUT_OPTIONS,
  NOTHING_FOUND_TITLE,
  PLAN_KIND_LABEL,
  SEARCH_CLEAR_LABEL,
  SEARCH_LABEL,
  SEARCH_PLACEHOLDER,
  STATE_FILTER_LABEL,
  STATE_FILTER_OPTIONS,
} from './artifactsCopy'
import { DEFAULT_VIEW, readView, writeView, type ArtifactsView } from './artifactsView'
import PlanArtifact from './PlanArtifact.vue'
import PlanCard from './PlanCard.vue'
import PlanImportDialog from './PlanImportDialog.vue'
import { planChip, planProgress, stagesOf } from './planProgress'
import { useArtifacts } from './useArtifacts'

const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col gap-4.5 overflow-y-auto ${PAGE_INSET_CLASSES}`
const AREA_CLASSES = 'relative flex flex-1 flex-col'
const GRID_CLASSES = 'relative grid grid-cols-[repeat(auto-fill,minmax(min(280px,100%),1fr))] gap-3.5 p-2'
const LIST_CLASSES = 'relative flex flex-col rounded-md border border-line bg-surface'
const GROUPS_CLASSES = 'relative flex flex-col gap-5'
const GROUP_TITLE_CLASSES = 'flex items-baseline gap-2 px-2 text-card-title text-ink'
const GROUP_TOGGLE_CLASSES = `flex w-fit cursor-pointer disabled:cursor-not-allowed items-baseline gap-2 rounded-sm px-2 text-card-title text-ink ${INK_FOCUS_CLASSES}`
const GROUP_COUNT_CLASSES = 'text-caption text-muted'
const SEARCH_CLASSES = `h-9 w-full min-w-0 rounded-md border border-line bg-surface pl-3 pr-8 text-control text-ink placeholder:text-faint ${INK_FOCUS_CLASSES}`
const SEARCH_CLEAR_CLASSES = `absolute right-1.5 top-1/2 -translate-y-1/2 cursor-pointer rounded-sm px-1.5 text-muted hover:text-ink ${INK_FOCUS_CLASSES}`
const CLOSED_GROUP = 'concluidos'
const EMPTY_AREA_CLASSES = 'relative flex flex-1 items-center justify-center'
const ALL_FILTER = 'todos'
const PLANS_FILTER = 'planos'
const PLAN_SOURCE = 'plano'
const DOCUMENT_SOURCE = 'documento'
const GENERATING_STATE = 'gerando'
const EMPTY_PROPOSAL = parseProposal('')

interface Opened {
  source: typeof PLAN_SOURCE | typeof DOCUMENT_SOURCE
  id: string
}

const backlog = useBacklog()
const itemPlan = useItemPlan()
const artifacts = useArtifacts()
const projectsStore = useProjects()
const navigation = useNavigation()

// Planos ainda sendo escritos (inclusive os importados) entram na grade para não sumirem até o agente responder.
const plans = computed(() =>
  backlog.items.flatMap((item) => {
    const proposal = itemPlan.proposalFor(item)
    const state = itemPlan.stateFor(item)
    return proposal || state === GENERATING_STATE
      ? [{ item, proposal: proposal ?? EMPTY_PROPOSAL, state, stages: stagesOf(backlog.items, item.id) }]
      : []
  }),
)
const documents = computed(() => [...artifacts.documents].sort((a, b) => b.updatedAt - a.updatedAt))
const total = computed(() => plans.value.length + documents.value.length)

function kindLabelOf(kind: string): string {
  return artifacts.kinds.find((info) => info.kind === kind)?.label ?? kind
}

const filter = ref<string>(ALL_FILTER)
const filterOptions = computed<{ value: string; label: string }[]>(() => {
  const options = [
    { value: ALL_FILTER, label: `${ARTIFACTS_ALL_LABEL} ${total.value}` },
    { value: PLANS_FILTER, label: `${ARTIFACTS_PLANS_LABEL} ${plans.value.length}` },
  ]
  for (const info of artifacts.kinds) {
    const count = documents.value.filter((document) => document.kind === info.kind).length
    if (count > 0) options.push({ value: info.kind, label: `${kindTabLabel(info.kind, info.label)} ${count}` })
  }
  return options
})
// Um tipo que ficou sem itens some do filtro; se estava selecionado, a tela volta para "Todos".
watch(filterOptions, (options) => {
  if (!options.some((option) => option.value === filter.value)) filter.value = ALL_FILTER
})

type PlanView = (typeof plans.value)[number]
interface CatalogEntry extends BrowsableArtifact {
  plan?: PlanView
  document?: ArtifactSummary
}

const view = ref<ArtifactsView>({ ...DEFAULT_VIEW })
// A busca, o estado, a visualização e o recolhido valem por projeto; a aba de tipo não é salva.
watch(
  () => projectsStore.selected?.id,
  (projectId) => {
    view.value = projectId === undefined ? { ...DEFAULT_VIEW } : readView(projectId)
  },
  { immediate: true },
)
watch(
  view,
  (next) => {
    const projectId = projectsStore.selected?.id
    if (projectId !== undefined) writeView(projectId, next)
  },
  { deep: true },
)

const catalog = computed<CatalogEntry[]>(() => [
  ...plans.value.map((plan) => ({
    key: `${PLAN_SOURCE}:${plan.item.id}`,
    id: plan.item.id,
    title: plan.item.title,
    summary: plan.proposal.summary,
    state: plan.state,
    column: plan.item.column,
    updatedAt: artifacts.planUpdatedAt[planArtifactId(plan.item.id)],
    plan,
  })),
  ...documents.value.map((document) => ({
    key: `${DOCUMENT_SOURCE}:${document.id}`,
    id: document.id,
    title: document.title,
    summary: document.summary,
    updatedAt: document.updatedAt,
    document,
  })),
])

function inTab(entry: CatalogEntry): boolean {
  if (filter.value === ALL_FILTER) return true
  if (filter.value === PLANS_FILTER) return entry.plan !== undefined
  return entry.document?.kind === filter.value
}

const filtering = computed(() => view.value.query.trim() !== '' || view.value.state !== ALL_STATES)
const groups = computed(() =>
  groupEntries(
    catalog.value.filter(
      (entry) => inTab(entry) && matchesQuery(entry, view.value.query) && matchesState(entry, view.value.state),
    ),
  ),
)
const showGroupTitles = computed(() => groups.value.length > 1 || groups.value[0]?.id === CLOSED_GROUP)
const stateModel = computed({
  get: (): StateFilter => view.value.state,
  set: (value) => {
    view.value.state = STATE_FILTER_OPTIONS.find((option) => option.value === value)?.value ?? ALL_STATES
  },
})

// Com busca ou filtro ativo, o grupo recolhido abre à força: um resultado escondido pareceria "nada encontrado".
function groupOpen(id: string): boolean {
  return id !== CLOSED_GROUP || filtering.value || view.value.doneOpen
}

function clearFilters(): void {
  view.value.query = ''
  view.value.state = ALL_STATES
}

function kindOf(entry: CatalogEntry): string {
  return entry.document ? kindLabelOf(entry.document.kind) : PLAN_KIND_LABEL
}

function chipOf(entry: CatalogEntry): ReturnType<typeof planChip> | undefined {
  return entry.plan ? planChip(entry.plan.state, entry.plan.item.column) : undefined
}

function progressOf(entry: CatalogEntry): string | undefined {
  if (entry.plan) {
    const progress = planProgress(entry.plan.stages)
    return progress ? `${progress.done} de ${progress.total} etapas` : undefined
  }
  const checklist = entry.document ? artifacts.progress[entry.document.id] : undefined
  return checklist && checklist.total > 0 ? `${checklist.done} de ${checklist.total} itens` : undefined
}

function openEntry(entry: CatalogEntry): void {
  if (entry.plan) opened.value = { source: PLAN_SOURCE, id: entry.plan.item.id }
  else if (entry.document) openDocument(entry.document.id)
}

const pendingPlanId = navigation.takePendingPlanId()
const pendingDocId = navigation.takePendingArtifactDocumentId()
const opened = ref<Opened | null>(
  pendingPlanId !== null
    ? { source: PLAN_SOURCE, id: pendingPlanId }
    : pendingDocId !== null
      ? { source: DOCUMENT_SOURCE, id: pendingDocId }
      : null,
)
if (pendingDocId !== null && artifacts.loadedProjectId !== null) {
  void artifacts.openDocument(pendingDocId)
}
watch(
  () => navigation.pendingArtifactPlanId,
  (nextId) => {
    if (nextId === null) return
    const id = navigation.takePendingPlanId()
    opened.value = id === null ? null : { source: PLAN_SOURCE, id }
  },
)
watch(
  () => navigation.pendingArtifactDocumentId,
  (nextId) => {
    if (nextId === null) return
    const id = navigation.takePendingArtifactDocumentId()
    if (id !== null) openDocument(id)
  },
)
watch(
  () => artifacts.loadedProjectId,
  (projectId) => {
    if (projectId !== null && opened.value?.source === DOCUMENT_SOURCE && artifacts.current?.id !== opened.value.id) {
      void artifacts.openDocument(opened.value.id)
    }
  },
)
const openedPlan = computed(() =>
  opened.value?.source === PLAN_SOURCE ? plans.value.find((plan) => plan.item.id === opened.value?.id) : undefined,
)
const openedDocument = computed(() =>
  opened.value?.source === DOCUMENT_SOURCE && artifacts.current?.id === opened.value.id ? artifacts.current : null,
)
const viewingDocument = computed(() => opened.value?.source === DOCUMENT_SOURCE)

function openDocument(id: string): void {
  opened.value = { source: DOCUMENT_SOURCE, id }
  void artifacts.openDocument(id)
}

function back(): void {
  artifacts.closeDocument()
  opened.value = null
}

// O documento aberto sumiu (excluído por outro lado): volta para a lista em vez de ficar numa tela vazia.
watch(
  () => artifacts.current,
  (current) => {
    if (viewingDocument.value && current === null && !artifacts.opening && !artifacts.error) back()
  },
)
watch(
  () => projectsStore.selected?.id,
  () => {
    opened.value = null
  },
)

const editor = ref<{ document?: ArtifactDocument } | null>(null)
const importing = ref(false)

function imported(itemId: string): void {
  importing.value = false
  artifacts.closeDocument()
  opened.value = { source: PLAN_SOURCE, id: itemId }
}

function openCreate(): void {
  editor.value = {}
}

function openEdit(): void {
  if (artifacts.current) editor.value = { document: artifacts.current }
}

function saved(document: ArtifactDocument): void {
  const created = editor.value?.document === undefined
  editor.value = null
  if (created) openDocument(document.id)
}
</script>

<template>
  <section :class="PAGE_CLASSES">
    <UiPageHeader :title="PAGE_LABELS.canvas">
      <template #actions>
        <UiButton
          v-if="!opened"
          variant="quiet"
          size="lg"
          data-testid="plan-import-open-dialog"
          :disabled="!projectsStore.selected"
          @press="importing = true"
        >
          {{ IMPORT_LABEL }}
        </UiButton>
        <UiCreateButton v-if="!opened" :disabled="!projectsStore.selected" @press="openCreate">
          {{ ARTIFACTS_NEW_LABEL }}
        </UiCreateButton>
      </template>
    </UiPageHeader>

    <template v-if="openedPlan">
      <div>
        <UiButton variant="quiet" size="sm" @press="back">{{ ARTIFACTS_BACK_LABEL }}</UiButton>
      </div>
      <PlanArtifact
        :key="openedPlan.item.id"
        :item="openedPlan.item"
        :proposal="openedPlan.proposal"
        :state="openedPlan.state"
        :stages="openedPlan.stages"
        @removed="back"
      />
    </template>

    <template v-else-if="viewingDocument">
      <div>
        <UiButton variant="quiet" size="sm" @press="back">{{ ARTIFACTS_BACK_LABEL }}</UiButton>
      </div>
      <ArtifactDocumentView
        v-if="openedDocument"
        :document="openedDocument"
        :kind-label="kindLabelOf(openedDocument.kind)"
        @edit="openEdit"
        @removed="back"
      />
      <p v-else-if="artifacts.opening" class="text-small text-muted">{{ ARTIFACTS_OPENING_TEXT }}</p>
      <UiNotice v-else-if="artifacts.error" :text="artifacts.error" @close="artifacts.clearFeedback()" />
    </template>

    <template v-else>
      <UiNotice v-if="artifacts.error" :text="artifacts.error" @close="artifacts.clearFeedback()" />
      <div class="flex flex-wrap items-center gap-3">
        <UiSegmented v-model="filter" :options="filterOptions" :label="ARTIFACTS_FILTER_LABEL" wrap />
        <div class="ml-auto flex min-w-0 flex-wrap items-center gap-2">
          <div class="relative w-64 max-w-full">
            <input
              v-model="view.query"
              type="text"
              :class="SEARCH_CLASSES"
              :aria-label="SEARCH_LABEL"
              :placeholder="SEARCH_PLACEHOLDER"
              data-testid="artifacts-search"
            />
            <button v-if="view.query" type="button" :class="SEARCH_CLEAR_CLASSES" :aria-label="SEARCH_CLEAR_LABEL" @click="view.query = ''">×</button>
          </div>
          <UiSelect v-model="stateModel" :options="STATE_FILTER_OPTIONS" :label="STATE_FILTER_LABEL" data-testid="artifacts-state" />
          <UiSegmented v-model="view.layout" :options="LAYOUT_OPTIONS" :label="LAYOUT_LABEL" />
        </div>
      </div>

      <div :class="AREA_CLASSES">
        <div aria-hidden="true" :class="DOT_GRID_CLASSES" />
        <div v-if="groups.length" :class="GROUPS_CLASSES">
          <section v-for="group in groups" :key="group.id" class="flex flex-col gap-2" :aria-label="GROUP_LABEL[group.id]" :data-testid="`artifacts-group-${group.id}`">
            <button
              v-if="group.id === CLOSED_GROUP"
              type="button"
              :class="GROUP_TOGGLE_CLASSES"
              :aria-expanded="groupOpen(group.id)"
              :aria-controls="`artifacts-group-body-${group.id}`"
              :disabled="filtering"
              @click="view.doneOpen = !view.doneOpen"
            >
              <span aria-hidden="true">{{ groupOpen(group.id) ? '▾' : '▸' }}</span>
              {{ GROUP_LABEL[group.id] }}
              <span :class="GROUP_COUNT_CLASSES">{{ group.entries.length }}</span>
            </button>
            <h3 v-else-if="showGroupTitles" :class="GROUP_TITLE_CLASSES">
              {{ GROUP_LABEL[group.id] }}
              <span :class="GROUP_COUNT_CLASSES">{{ group.entries.length }}</span>
            </h3>
            <div v-show="groupOpen(group.id)" :id="`artifacts-group-body-${group.id}`">
              <div v-if="view.layout === 'lista'" :class="LIST_CLASSES">
                <ArtifactListRow
                  v-for="entry in group.entries"
                  :key="entry.key"
                  :id="entry.id"
                  :title="entry.title"
                  :kind-label="kindOf(entry)"
                  :chip="chipOf(entry)"
                  :progress="progressOf(entry)"
                  :updated-at="entry.updatedAt"
                  @open="openEntry(entry)"
                />
              </div>
              <div v-else :class="GRID_CLASSES">
                <template v-for="entry in group.entries" :key="entry.key">
                  <PlanCard
                    v-if="entry.plan"
                    :item="entry.plan.item"
                    :proposal="entry.plan.proposal"
                    :state="entry.plan.state"
                    :stages="entry.plan.stages"
                    @open="openEntry(entry)"
                  />
                  <ArtifactCard
                    v-else-if="entry.document"
                    :summary="entry.document"
                    :kind-label="kindLabelOf(entry.document.kind)"
                    :progress="artifacts.progress[entry.document.id]"
                    @open="openEntry(entry)"
                  />
                </template>
              </div>
            </div>
          </section>
        </div>
        <div v-else :class="EMPTY_AREA_CLASSES">
          <p v-if="artifacts.loading && total === 0" class="text-small text-muted">{{ ARTIFACTS_LOADING_TEXT }}</p>
          <UiEmpty v-else-if="total === 0" :title="ARTIFACTS_EMPTY_TITLE" />
          <UiEmpty v-else :title="NOTHING_FOUND_TITLE">
            <template #action>
              <UiButton variant="quiet" size="sm" data-testid="artifacts-clear-filters" @press="clearFilters">{{ CLEAR_FILTERS_LABEL }}</UiButton>
            </template>
          </UiEmpty>
        </div>
      </div>
    </template>

    <ArtifactEditorDialog
      v-if="editor"
      :document="editor.document"
      @close="editor = null"
      @saved="saved"
    />
    <PlanImportDialog v-if="importing" @close="importing = false" @done="imported" />
  </section>
</template>
