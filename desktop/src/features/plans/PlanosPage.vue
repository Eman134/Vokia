<script setup lang="ts">
import { computed, shallowRef } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import { useProjects } from '../projects/useProjects'
import PlanDetail from './PlanDetail.vue'
import PlanList from './PlanList.vue'
import {
  INITIAL_PLAN,
  NEW_PLAN_LABEL,
  PLAN_FILES,
  PLAN_STORAGE_TEXT,
  PLANS,
  SCOPE_CRUMB_PREFIX,
  SCOPE_SUBTITLE_PREFIX,
  SUB_PLAN_SUBTITLE_PREFIX,
  type SamplePlan,
} from './planosSample'

const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col gap-4.5 overflow-y-auto ${PAGE_INSET_CLASSES}`
const FILE_CLASSES = 'rounded-control bg-surface-2 px-2.5 py-1.25 font-mono text-small text-ink'
const BODY_CLASSES = 'flex flex-wrap items-start gap-4'

const selected = shallowRef<SamplePlan>(INITIAL_PLAN)

const parent = computed(() => PLANS.find((plan) => plan.id === selected.value.parentId))
const crumb = computed(() =>
  parent.value
    ? `${parent.value.name} › ${selected.value.name}`
    : `${SCOPE_CRUMB_PREFIX} ${selected.value.scope}`,
)
const projectsStore = useProjects()
const projectLabel = computed(() => [projectsStore.selected?.name, crumb.value].filter(Boolean).join(' · '))
const subtitle = computed(() => {
  const lead = parent.value
    ? `${SUB_PLAN_SUBTITLE_PREFIX} "${parent.value.name}". `
    : `${SCOPE_SUBTITLE_PREFIX} ${selected.value.scope}. `
  return lead + PLAN_STORAGE_TEXT
})
const files = computed(() => {
  const prefix = selected.value.scope === 'feature' ? '' : `${selected.value.id}/`
  return PLAN_FILES.map((name) => `${prefix}${name}`)
})

function select(plan: SamplePlan) {
  selected.value = plan
}
</script>

<template>
  <section :class="PAGE_CLASSES">
    <UiPageHeader :title="selected.name" :project-name="projectLabel" :subtitle="subtitle">
      <template #actions>
        <span v-for="file in files" :key="file" :class="FILE_CLASSES">{{ file }}</span>
        <UiButton variant="primary" size="lg" disabled aria-disabled="true">{{ NEW_PLAN_LABEL }}</UiButton>
      </template>
    </UiPageHeader>

    <div :class="BODY_CLASSES">
      <PlanList :plans="PLANS" :selected-id="selected.id" @select="select" />
      <PlanDetail :plan="selected" />
    </div>
  </section>
</template>
