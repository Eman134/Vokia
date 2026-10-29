<script setup lang="ts">
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import {
  ACTION_ID,
  type ActionLabel,
  colorVar,
  DEFAULT_FEATURE_COLOR,
  FEATURE_COLOR,
  STAGE_BADGE,
  STAGE_CHIP,
  STATE_ACTION,
  STATE_BADGE,
  STATE_CHIP,
  TAG_COLOR,
  TAG_LABEL,
  TRIAGE_STEP_COUNT,
  YOU_INITIAL,
} from './backlogLabels'
import type { BacklogCardAction, BacklogCardDensity, BacklogItem, BacklogTag } from './backlogTypes'

const props = withDefaults(defineProps<{ item: BacklogItem; density?: BacklogCardDensity }>(), {
  density: 'normal',
})
const emit = defineEmits<{ open: []; action: [action: BacklogCardAction] }>()

const TAG_TINT_PERCENT = 13
const DONE_SEGMENTS = 1
const VOCE = 'voce'

const isCompact = computed(() => props.density === 'compacta')
const featureColor = computed(() => colorVar(FEATURE_COLOR[props.item.feature ?? ''] ?? DEFAULT_FEATURE_COLOR))

const chip = computed(() => {
  const { stage, state } = props.item
  return stage ? { label: STAGE_CHIP[stage], status: STAGE_BADGE[stage] } : { label: STATE_CHIP[state], status: STATE_BADGE[state] }
})

const action = computed<{ label: ActionLabel; id: BacklogCardAction } | undefined>(() => {
  const label = props.item.stage ? (props.item.stage === 'review' ? 'Ver diff' : undefined) : STATE_ACTION[props.item.state]
  return label && { label, id: ACTION_ID[label] }
})

const progress = computed(() => {
  const { stage, state, children, triageStep } = props.item
  if (stage === 'done') {
    const total = Math.max(children?.total ?? 0, DONE_SEGMENTS)
    return { total, filled: total, fillClass: 'bg-ok', showText: children !== undefined && children.total > 0 }
  }
  if (children) {
    return { total: children.total, filled: Math.min(children.done, children.total), fillClass: 'bg-ok', showText: true }
  }
  if (state === 'triagem') return { total: TRIAGE_STEP_COUNT, filled: triageStep ?? 0, fillClass: 'bg-accent', showText: false }
  return undefined
})

const percent = computed(() => {
  if (!progress.value || progress.value.total === 0) return 0
  return Math.round((progress.value.filled / progress.value.total) * 100)
})

const authorInitial = computed(() => (props.item.who === VOCE ? YOU_INITIAL : props.item.who.charAt(0).toUpperCase()))

const showPlan = computed(() => !isCompact.value && props.item.state !== 'vazio' && Boolean(props.item.plan))

function tagPillStyle(tag: BacklogTag) {
  const color = colorVar(TAG_COLOR[tag])
  return { color, backgroundColor: `color-mix(in srgb, ${color} ${TAG_TINT_PERCENT}%, transparent)` }
}
</script>

<template>
  <article
    class="flex cursor-pointer flex-col gap-2 rounded-card border border-line bg-surface p-3.5"
    :class="isCompact && 'border-l-4'"
    :style="isCompact ? { borderLeftColor: featureColor } : undefined"
    @click="emit('open')"
  >
    <header class="flex items-center gap-2 text-caption text-muted">
      <span>{{ item.id }}</span>
      <UiBadge v-if="item.type === 'feature'" status="backlog" label="feature" />
      <UiBadge v-if="item.isNew" status="lote" label="novo" />
      <UiBadge class="ml-auto" :status="chip.status" :label="chip.label" />
    </header>

    <p v-if="item.parent && !isCompact" class="text-caption text-muted">↳ {{ item.parent }}</p>
    <h3 class="text-control font-semibold text-ink" :class="isCompact ? 'truncate' : 'wrap-break-word'">
      {{ item.title }}
    </h3>

    <p v-if="!isCompact && item.description" class="text-caption text-muted">{{ item.description }}</p>
    <p v-if="showPlan" class="wrap-break-word whitespace-pre-wrap text-caption text-ink">{{ item.plan }}</p>

    <ul v-if="isCompact" class="flex items-center gap-1.5">
      <li
        v-for="tag in item.tags"
        :key="tag"
        class="size-2 rounded-full"
        :style="{ backgroundColor: colorVar(TAG_COLOR[tag]) }"
        :title="TAG_LABEL[tag]"
      />
    </ul>
    <ul v-else-if="item.tags.length" class="flex flex-wrap gap-1.5">
      <li
        v-for="tag in item.tags"
        :key="tag"
        class="inline-flex h-5.5 items-center rounded-chip px-2 text-meta font-medium whitespace-nowrap"
        :style="tagPillStyle(tag)"
      >
        {{ TAG_LABEL[tag] }}
      </li>
    </ul>

    <div v-if="progress" class="flex flex-col gap-1.5">
      <p v-if="progress.showText" class="flex justify-between text-caption text-muted">
        <span>{{ item.children?.total }} micro-tarefas · {{ progress.filled }} feitas</span>
        <span>{{ percent }}%</span>
      </p>
      <div class="flex gap-1">
        <span
          v-for="segment in progress.total"
          :key="segment"
          class="h-1.5 flex-1 rounded-full"
          :class="segment <= progress.filled ? progress.fillClass : 'bg-surface-2'"
        />
      </div>
    </div>

    <footer v-if="!isCompact" class="flex items-center gap-2 text-caption text-muted">
      <span
        class="grid size-5 shrink-0 place-items-center rounded-full border border-line bg-surface-2 text-tiny font-semibold text-ink"
      >
        {{ authorInitial }}
      </span>
      <span>{{ item.by }}</span>
      <UiButton v-if="action" class="ml-auto" variant="primary" size="sm" @click.stop @press="emit('action', action.id)">
        {{ action.label }}
      </UiButton>
    </footer>
  </article>
</template>
