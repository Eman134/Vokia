<script setup lang="ts">
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import {
  PLAN_GROUPS,
  PLAN_LIST_TITLE,
  PLAN_STATUS_BADGE,
  PLAN_STATUS_LABEL,
  type PlanStatus,
  type SamplePlan,
} from './planosSample'

const LIST_CLASSES =
  'flex w-[280px] shrink-0 flex-col gap-0.5 rounded-dialog border border-line bg-surface p-2'
const LIST_HEADER_CLASSES = 'flex items-center px-2.5 py-1.5'
const GROUP_LABEL_CLASSES = 'px-2.5 pt-2.5 pb-1 text-tiny font-semibold tracking-[0.03em] text-faint'
const ROW_CLASSES =
  'flex w-full items-center gap-2.25 rounded-control-lg px-2.5 py-2 text-left text-ink focus:outline-2 focus:outline-solid focus:outline-accent'
const SELECTED_ROW_CLASSES = 'bg-accent-soft'
const CONNECTOR_CLASSES =
  '-mt-2.5 ml-1 h-4.5 w-3.5 flex-none rounded-bl-chip border-b-[1.5px] border-l-[1.5px] border-line'
const DOT_CLASSES = 'size-2 flex-none rounded-full'
const PLAN_DOT_CLASSES: Record<PlanStatus, string> = {
  run: 'bg-ok',
  review: 'bg-accent',
  draft: 'bg-faint',
  done: 'bg-line',
}
const TEXT_CLASSES = 'flex min-w-0 flex-1 flex-col gap-0.5'
const NAME_CLASSES = 'truncate text-label font-medium'
const META_CLASSES = 'truncate text-meta text-muted'

const props = defineProps<{
  plans: readonly SamplePlan[]
  selectedId: string
}>()
const emit = defineEmits<{ select: [plan: SamplePlan] }>()

interface ListEntry {
  plan: SamplePlan
  child: boolean
}

const groups = computed(() =>
  PLAN_GROUPS.map(({ scope, label }) => ({
    label,
    entries: props.plans
      .filter((plan) => plan.scope === scope && !plan.parentId)
      .flatMap<ListEntry>((plan) => [
        { plan, child: false },
        ...props.plans.filter((child) => child.parentId === plan.id).map((child) => ({ plan: child, child: true })),
      ]),
  })).filter((group) => group.entries.length > 0),
)
</script>

<template>
  <aside aria-label="Planos" :class="LIST_CLASSES">
    <div :class="LIST_HEADER_CLASSES">
      <span class="text-small text-faint">{{ PLAN_LIST_TITLE }}</span>
      <span class="ml-auto text-small text-muted">{{ plans.length }}</span>
    </div>
    <div v-for="group in groups" :key="group.label">
      <p :class="GROUP_LABEL_CLASSES">{{ group.label }}</p>
      <button
        v-for="entry in group.entries"
        :key="entry.plan.id"
        type="button"
        :class="[ROW_CLASSES, entry.plan.id === selectedId && SELECTED_ROW_CLASSES]"
        :aria-pressed="entry.plan.id === selectedId"
        @click="emit('select', entry.plan)"
      >
        <span v-if="entry.child" :class="CONNECTOR_CLASSES" />
        <span :class="[DOT_CLASSES, PLAN_DOT_CLASSES[entry.plan.status]]" />
        <span :class="TEXT_CLASSES">
          <span :class="NAME_CLASSES">{{ entry.plan.name }}</span>
          <span :class="META_CLASSES">{{ entry.plan.meta }}</span>
        </span>
        <UiBadge :status="PLAN_STATUS_BADGE[entry.plan.status]" :label="PLAN_STATUS_LABEL[entry.plan.status]" />
      </button>
    </div>
  </aside>
</template>
