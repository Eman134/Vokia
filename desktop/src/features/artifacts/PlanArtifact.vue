<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import UiNotice from '../../ui/UiNotice.vue'
import UiProgress from '../../ui/UiProgress.vue'
import { STATE_BADGE, STATE_CHIP } from '../backlog/backlogLabels'
import type { BacklogItem, BacklogState } from '../backlog/backlogTypes'
import PlanBreakdown from '../backlog/PlanBreakdown.vue'
import { breakdownOf } from '../backlog/planBreakdown'
import type { ParsedProposal } from '../backlog/planProposal'
import { useBacklog } from '../backlog/useBacklog'
import { useItemPlan } from '../backlog/useItemPlan'
import { useProjects } from '../projects/useProjects'
import { agentName, DEFAULT_RECIPIENT } from '../team/agents'
import { useRoster } from '../team/useRoster'
import {
  fetchPlanArtifact,
  savePlanMarkdown,
  setArtifactCriterion,
  type ArtifactCriterion,
  type PlanArtifactDetail,
} from './artifactApi'
import {
  PLAN_CARD_CRITERIA_TITLE,
  PLAN_CRITERIA_TITLE,
  PLAN_DOWNLOAD_ARIA,
  PLAN_DOWNLOAD_LABEL,
  PLAN_EDIT_LABEL,
  PLAN_LOADING_TEXT,
  PLAN_RETRY_LABEL,
  PLAN_RISKS_TITLE,
  PLAN_SAVED_NOTICE,
  PLAN_STAGES_TITLE,
} from './artifactsCopy'
import PlanEditorDialog from './PlanEditorDialog.vue'
import { planFileName, planToMarkdown } from './planMarkdown'
import PlanStageRow from './PlanStageRow.vue'
import { planProgress } from './planProgress'

const WRAP_CLASSES = 'wrap-break-word'
const SECTION_CLASSES = 'flex min-w-0 flex-col gap-3'
const SECTION_TITLE_CLASSES = 'text-small font-semibold text-muted'
const SUBSECTION_TITLE_CLASSES = 'text-caption font-semibold text-muted'
const CRITERION_CLASSES = 'flex cursor-pointer items-start gap-2.5 text-control text-ink'
const CHECKBOX_CLASSES = 'mt-1 size-4 flex-none cursor-pointer'
const GENERATING_STATE: BacklogState = 'gerando'

const props = defineProps<{
  item: BacklogItem
  proposal: ParsedProposal
  state: BacklogState
  stages: BacklogItem[]
}>()

const backlog = useBacklog()
const itemPlan = useItemPlan()
const projects = useProjects()
const roster = useRoster()
const leaderName = computed(() => agentName(DEFAULT_RECIPIENT))

const artifact = ref<PlanArtifactDetail | null>(null)
const loadError = ref<string | null>(null)
const notice = ref<string | null>(null)
const downloading = ref(false)
const editing = ref(false)
let loadSeq = 0

const progress = computed(() => planProgress(props.stages))
const breakdown = computed(() =>
  breakdownOf(props.item, props.proposal, [], (agentId) => roster.profileOf(agentId)?.model),
)
const expandedSteps = ref(new Set<string>(breakdown.value.steps[0] ? [breakdown.value.steps[0].id] : []))
const loading = computed(() => artifact.value === null && loadError.value === null && props.state !== GENERATING_STATE)

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

// Respostas fora de ordem (outra carga começou depois) são descartadas.
async function load(): Promise<void> {
  const projectId = projects.selected?.id
  if (projectId === undefined || props.state === GENERATING_STATE) return
  const seq = (loadSeq += 1)
  try {
    const fresh = await fetchPlanArtifact(projectId, props.item.id)
    if (seq !== loadSeq) return
    artifact.value = fresh
    loadError.value = null
  } catch (error) {
    if (seq === loadSeq) loadError.value = messageOf(error)
  }
}

onMounted(load)
// set_artifact_criterion e update_item_plan avisam por backlog-changed, que troca a lista inteira de itens.
watch(() => backlog.items, load)
watch(() => props.state, load)

async function toggleCriterion(criterion: ArtifactCriterion): Promise<void> {
  const projectId = projects.selected?.id
  const current = artifact.value
  if (projectId === undefined || !current) return
  const done = !criterion.done
  criterion.done = done
  try {
    await setArtifactCriterion(projectId, current.id, criterion.id, done)
  } catch (error) {
    criterion.done = !done
    notice.value = messageOf(error)
  }
}

async function download(): Promise<void> {
  const current = artifact.value
  if (!current || downloading.value) return
  downloading.value = true
  notice.value = null
  try {
    const content = planToMarkdown({
      item: props.item,
      proposal: props.proposal,
      rawText: current.detail || itemPlan.planOf(props.item.id).proposal || '',
      stages: props.stages,
      criteria: current.criteria,
      state: props.state,
    })
    const path = await savePlanMarkdown(planFileName(props.item), content)
    if (path) notice.value = PLAN_SAVED_NOTICE(path)
  } catch (error) {
    notice.value = messageOf(error)
  } finally {
    downloading.value = false
  }
}

function saved(fresh: PlanArtifactDetail): void {
  artifact.value = fresh
  editing.value = false
}
</script>

<template>
  <article class="flex min-w-0 flex-col gap-6" :aria-label="`Plano de ${item.title}`">
    <header class="flex min-w-0 flex-col gap-2">
      <p class="text-caption text-muted">Artefatos › Plano · <span class="font-mono">{{ item.id }}</span></p>
      <div class="flex flex-col items-start gap-2 sm:flex-row sm:items-center sm:gap-2.5">
        <h3 :class="['min-w-0 flex-1 text-lead text-ink', WRAP_CLASSES]">{{ item.title }}</h3>
        <div class="flex flex-wrap items-center gap-2">
          <UiBadge :status="STATE_BADGE[state]" :label="STATE_CHIP[state]" />
          <UiButton
            variant="quiet"
            size="sm"
            class="gap-1.5"
            data-testid="plan-download"
            :aria-label="PLAN_DOWNLOAD_ARIA"
            :disabled="!artifact || downloading"
            @press="download"
          >
            <svg
              class="size-4"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <path d="M12 15V3" />
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <path d="m7 10 5 5 5-5" />
            </svg>
            <span>{{ PLAN_DOWNLOAD_LABEL }}</span>
          </UiButton>
          <UiButton variant="quiet" size="sm" data-testid="plan-edit" :disabled="!artifact" @press="editing = true">
            {{ PLAN_EDIT_LABEL }}
          </UiButton>
        </div>
      </div>
      <p v-if="proposal.estimate" class="text-small text-muted">Estimativa: {{ proposal.estimate }}</p>
      <p v-if="proposal.summary" :class="['whitespace-pre-wrap text-body text-ink', WRAP_CLASSES]">{{ proposal.summary }}</p>
      <div v-if="progress" class="flex flex-col gap-1.5">
        <div class="flex flex-wrap justify-between gap-x-3 text-meta text-muted">
          <span>{{ progress.done }} de {{ progress.total }} etapas feitas</span>
          <span>{{ progress.percent }}%</span>
        </div>
        <UiProgress :value="progress.percent" />
      </div>
    </header>

    <UiNotice v-if="notice" :text="notice" data-testid="plan-notice" @close="notice = null" />
    <div v-if="loadError" class="flex min-w-0 flex-wrap items-center gap-3 rounded-control bg-surface-2 py-2 pl-3.5 pr-2 text-caption text-ink" role="alert">
      <p class="min-w-0 flex-1">{{ loadError }}</p>
      <UiButton variant="quiet" size="sm" data-testid="plan-retry" @press="load">{{ PLAN_RETRY_LABEL }}</UiButton>
    </div>
    <p v-else-if="loading" class="text-small text-muted">{{ PLAN_LOADING_TEXT }}</p>

    <section :aria-label="PLAN_STAGES_TITLE" :class="SECTION_CLASSES">
      <h4 :class="SECTION_TITLE_CLASSES">{{ PLAN_STAGES_TITLE }}</h4>
      <div v-if="stages.length" class="flex flex-col gap-3">
        <PlanStageRow v-for="stage in stages" :key="stage.id" :stage="stage" :parent-title="item.title" />
      </div>
      <PlanBreakdown
        v-else-if="breakdown.steps.length"
        v-model:expanded="expandedSteps"
        :breakdown="breakdown"
        :leader="leaderName"
      />
    </section>

    <section
      v-if="artifact?.criteria.length || item.criteria?.length"
      :aria-label="PLAN_CRITERIA_TITLE"
      :class="SECTION_CLASSES"
    >
      <h4 :class="SECTION_TITLE_CLASSES">{{ PLAN_CRITERIA_TITLE }}</h4>
      <ul v-if="artifact?.criteria.length" class="flex flex-col gap-2" data-testid="plan-criteria">
        <li v-for="criterion in artifact.criteria" :key="criterion.id">
          <label :class="CRITERION_CLASSES">
            <input type="checkbox" :checked="criterion.done" :class="CHECKBOX_CLASSES" @change="toggleCriterion(criterion)" />
            <span :class="['min-w-0 flex-1', WRAP_CLASSES, criterion.done && 'text-muted line-through']">{{ criterion.text }}</span>
          </label>
        </li>
      </ul>

      <div v-if="item.criteria?.length" class="flex flex-col gap-2">
        <h5 :class="SUBSECTION_TITLE_CLASSES">{{ PLAN_CARD_CRITERIA_TITLE }}</h5>
        <ul class="flex flex-col gap-2">
          <li v-for="criterion in item.criteria" :key="criterion.id">
            <label :class="CRITERION_CLASSES">
              <input
                type="checkbox"
                :checked="criterion.done"
                :class="CHECKBOX_CLASSES"
                @change="backlog.toggleCriterion(item.id, criterion.id)"
              />
              <span :class="['min-w-0 flex-1', WRAP_CLASSES, criterion.done && 'text-muted line-through']">{{ criterion.text }}</span>
            </label>
          </li>
        </ul>
      </div>
    </section>

    <section v-if="proposal.risk" :aria-label="PLAN_RISKS_TITLE" :class="SECTION_CLASSES">
      <h4 :class="SECTION_TITLE_CLASSES">{{ PLAN_RISKS_TITLE }}</h4>
      <p :class="['text-small text-ink', WRAP_CLASSES]">{{ proposal.risk }}</p>
    </section>

    <PlanEditorDialog
      v-if="editing && artifact"
      :item="item"
      :artifact="artifact"
      :stages="stages"
      @close="editing = false"
      @saved="saved"
    />
  </article>
</template>
