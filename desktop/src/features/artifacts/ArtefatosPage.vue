<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiCreateButton from '../../ui/UiCreateButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import { DOT_GRID_CLASSES } from '../../ui/dotGrid'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiNotice from '../../ui/UiNotice.vue'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { PAGE_LABELS } from '../../shell/railNav'
import { useNavigation } from '../../shell/useNavigation'
import { useBacklog } from '../backlog/useBacklog'
import { parseProposal } from '../backlog/planProposal'
import { useItemPlan } from '../backlog/useItemPlan'
import { useProjects } from '../projects/useProjects'
import type { ArtifactDocument } from './artifactApi'
import ArtifactCard from './ArtifactCard.vue'
import ArtifactDocumentView from './ArtifactDocumentView.vue'
import ArtifactEditorDialog from './ArtifactEditorDialog.vue'
import {
  ARTIFACTS_ALL_LABEL,
  ARTIFACTS_BACK_LABEL,
  ARTIFACTS_EMPTY_TITLE,
  ARTIFACTS_FILTER_LABEL,
  ARTIFACTS_LOADING_TEXT,
  ARTIFACTS_NEW_LABEL,
  ARTIFACTS_OPENING_TEXT,
  ARTIFACTS_PLANS_LABEL,
  IMPORT_LABEL,
  kindTabLabel,
} from './artifactsCopy'
import PlanArtifact from './PlanArtifact.vue'
import PlanCard from './PlanCard.vue'
import PlanImportDialog from './PlanImportDialog.vue'
import { stagesOf } from './planProgress'
import { useArtifacts } from './useArtifacts'

const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col gap-4.5 overflow-y-auto ${PAGE_INSET_CLASSES}`
const AREA_CLASSES = 'relative flex flex-1 flex-col'
const GRID_CLASSES = 'relative grid grid-cols-[repeat(auto-fill,minmax(min(280px,100%),1fr))] gap-3.5 p-2'
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
const projectName = computed(() => projectsStore.selected?.name ?? '')

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

const visiblePlans = computed(() => (filter.value === ALL_FILTER || filter.value === PLANS_FILTER ? plans.value : []))
const visibleDocuments = computed(() => {
  if (filter.value === ALL_FILTER) return documents.value
  if (filter.value === PLANS_FILTER) return []
  return documents.value.filter((document) => document.kind === filter.value)
})

const pendingPlanId = navigation.takePendingPlanId()
const opened = ref<Opened | null>(pendingPlanId === null ? null : { source: PLAN_SOURCE, id: pendingPlanId })
watch(
  () => navigation.pendingArtifactPlanId,
  (nextId) => {
    if (nextId === null) return
    const id = navigation.takePendingPlanId()
    opened.value = id === null ? null : { source: PLAN_SOURCE, id }
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
    <UiPageHeader :title="PAGE_LABELS.canvas" :project-name="projectName">
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
      </div>

      <div :class="AREA_CLASSES">
      <div aria-hidden="true" :class="DOT_GRID_CLASSES" />
      <div v-if="visiblePlans.length || visibleDocuments.length" :class="GRID_CLASSES">
        <PlanCard
          v-for="plan in visiblePlans"
          :key="plan.item.id"
          :item="plan.item"
          :proposal="plan.proposal"
          :state="plan.state"
          :stages="plan.stages"
          @open="opened = { source: PLAN_SOURCE, id: plan.item.id }"
        />
        <ArtifactCard
          v-for="document in visibleDocuments"
          :key="document.id"
          :summary="document"
          :kind-label="kindLabelOf(document.kind)"
          :progress="artifacts.progress[document.id]"
          @open="openDocument(document.id)"
        />
      </div>
      <div v-else :class="EMPTY_AREA_CLASSES">
        <p v-if="artifacts.loading" class="text-small text-muted">{{ ARTIFACTS_LOADING_TEXT }}</p>
        <UiEmpty v-else :title="ARTIFACTS_EMPTY_TITLE" />
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
