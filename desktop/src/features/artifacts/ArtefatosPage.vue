<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { PAGE_LABELS } from '../../shell/railNav'
import { useBacklog } from '../backlog/useBacklog'
import { useItemPlan } from '../backlog/useItemPlan'
import { useProjects } from '../projects/useProjects'
import PlanArtifact from './PlanArtifact.vue'
import PlanCard from './PlanCard.vue'
import { stagesOf } from './planProgress'

const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col gap-4.5 overflow-y-auto ${PAGE_INSET_CLASSES}`
const PLAN_GRID_CLASSES = 'grid grid-cols-[repeat(auto-fill,minmax(min(280px,100%),1fr))] gap-3.5'
const EMPTY_AREA_CLASSES = 'flex flex-1 items-center justify-center'
const EMPTY_TITLE = 'Nenhum artefato'
const BACK_LABEL = 'Voltar'
const NEW_ARTIFACT_LABEL = '+ Novo artefato'
const ALL_FILTER = 'todos'
const PLANS_FILTER = 'planos'

type PageFilter = typeof ALL_FILTER | typeof PLANS_FILTER

const backlog = useBacklog()
const itemPlan = useItemPlan()
const projectsStore = useProjects()
const projectName = computed(() => projectsStore.selected?.name ?? '')

const plans = computed(() =>
  backlog.items.flatMap((item) => {
    const proposal = itemPlan.proposalFor(item)
    return proposal
      ? [{ item, proposal, state: itemPlan.stateFor(item), stages: stagesOf(backlog.items, item.id) }]
      : []
  }),
)

const filter = ref<PageFilter>(ALL_FILTER)
const filterOptions = computed<{ value: PageFilter; label: string }[]>(() => [
  { value: ALL_FILTER, label: `Todos ${plans.value.length}` },
  { value: PLANS_FILTER, label: `Planos ${plans.value.length}` },
])
const countLabel = computed(() => `${plans.value.length} ${plans.value.length === 1 ? 'artefato' : 'artefatos'}`)

const openedPlanId = ref<string | null>(null)
const openedPlan = computed(() => plans.value.find((plan) => plan.item.id === openedPlanId.value))
</script>

<template>
  <section :class="PAGE_CLASSES">
    <UiPageHeader :title="PAGE_LABELS.canvas" :project-name="projectName">
      <template #actions>
        <UiButton v-if="!openedPlan" variant="primary" size="lg" disabled aria-disabled="true">
          {{ NEW_ARTIFACT_LABEL }}
        </UiButton>
      </template>
    </UiPageHeader>

    <template v-if="openedPlan">
      <div>
        <UiButton variant="quiet" size="sm" @click="openedPlanId = null">{{ BACK_LABEL }}</UiButton>
      </div>
      <PlanArtifact
        :item="openedPlan.item"
        :proposal="openedPlan.proposal"
        :state="openedPlan.state"
        :stages="openedPlan.stages"
      />
    </template>

    <template v-else>
      <div class="flex flex-wrap items-center gap-3">
        <UiSegmented v-model="filter" :options="filterOptions" label="Tipo de artefato" wrap />
        <span class="ml-auto text-caption text-muted">{{ countLabel }}</span>
      </div>

      <div v-if="plans.length" :class="PLAN_GRID_CLASSES">
        <PlanCard
          v-for="plan in plans"
          :key="plan.item.id"
          :item="plan.item"
          :proposal="plan.proposal"
          :state="plan.state"
          :stages="plan.stages"
          @open="openedPlanId = plan.item.id"
        />
      </div>
      <div v-else :class="EMPTY_AREA_CLASSES">
        <UiEmpty :title="EMPTY_TITLE" />
      </div>
    </template>
  </section>
</template>
