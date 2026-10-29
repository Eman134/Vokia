<script setup lang="ts">
import { computed, shallowRef } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import { CURRENT_PROJECT, SAMPLE_DATA_LABEL } from '../../shell/railNav'
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

const PAGE_CLASSES = 'flex min-w-0 flex-1 flex-col gap-4.5 overflow-y-auto pt-7 pr-8 pb-[150px] pl-2'
const HEADER_CLASSES = 'flex flex-wrap items-end gap-4'
const TITLE_CLASSES = 'mt-1 text-[32px] font-bold tracking-[-0.03em] text-ink'
const SUBTITLE_CLASSES = 'mt-1 max-w-[70ch] text-body text-muted'
const ACTIONS_CLASSES = 'ml-auto flex flex-wrap items-center gap-2'
const FILE_CLASSES = 'rounded-control bg-surface-2 px-2.5 py-1.25 font-mono text-small text-ink'
const BODY_CLASSES = 'flex flex-wrap items-start gap-4'

const selected = shallowRef<SamplePlan>(INITIAL_PLAN)

const parent = computed(() => PLANS.find((plan) => plan.id === selected.value.parentId))
const crumb = computed(() =>
  parent.value
    ? `${parent.value.name} › ${selected.value.name}`
    : `${SCOPE_CRUMB_PREFIX} ${selected.value.scope}`,
)
const contextLabel = computed(() => `${CURRENT_PROJECT} · ${crumb.value} · ${SAMPLE_DATA_LABEL}`)
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
    <header :class="HEADER_CLASSES">
      <div>
        <p class="text-caption text-muted">{{ contextLabel }}</p>
        <h1 :class="TITLE_CLASSES">{{ selected.name }}</h1>
        <p :class="SUBTITLE_CLASSES">{{ subtitle }}</p>
      </div>
      <div :class="ACTIONS_CLASSES">
        <span v-for="file in files" :key="file" :class="FILE_CLASSES">{{ file }}</span>
        <UiButton variant="primary" size="lg" disabled aria-disabled="true">{{ NEW_PLAN_LABEL }}</UiButton>
      </div>
    </header>

    <div :class="BODY_CLASSES">
      <PlanList :plans="PLANS" :selected-id="selected.id" @select="select" />
      <PlanDetail :plan="selected" />
    </div>
  </section>
</template>
