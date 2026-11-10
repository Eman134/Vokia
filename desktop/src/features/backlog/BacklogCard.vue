<script setup lang="ts">
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import {
  ACTION_ID,
  ACTION_TITLE,
  type ActionLabel,
  colorVar,
  COLUMN_CHIP,
  DEFAULT_FEATURE_COLOR,
  FEATURE_COLOR,
  STATE_ACTION,
  STATE_BADGE,
  STATE_CHIP,
  TRIAGE_STEP_COUNT,
} from './backlogLabels'
import { tagColor, tagLabel, tagTextColor } from './tags'
import { YOU_INITIAL } from '../team/portraitArt'
import { DEFAULT_COLUMN, type BacklogCardAction, type BacklogCardDensity, type BacklogColumn, type BacklogItem, type BacklogState, type BacklogTag } from './backlogTypes'

const props = withDefaults(defineProps<{ item: BacklogItem; column: BacklogColumn; state: BacklogState; density?: BacklogCardDensity; planLine?: string }>(), {
  density: 'normal',
})
const emit = defineEmits<{ open: []; action: [action: BacklogCardAction] }>()

const TAG_TINT_PERCENT = 13
const DONE_SEGMENTS = 1
const DONE_COLUMN: BacklogColumn = 'Done'
const VOCE = 'voce'

const isCompact = computed(() => props.density === 'compacta')
const featureColor = computed(() => colorVar(FEATURE_COLOR[props.item.feature ?? ''] ?? DEFAULT_FEATURE_COLOR))

const chip = computed(
  () => COLUMN_CHIP[props.column] ?? { label: STATE_CHIP[props.state], status: STATE_BADGE[props.state] },
)

const action = computed<{ label: ActionLabel; id: BacklogCardAction } | undefined>(() => {
  if (props.item.error || props.column !== DEFAULT_COLUMN) return undefined
  const label = STATE_ACTION[props.state]
  return label && { label, id: ACTION_ID[label] }
})

const progress = computed(() => {
  const { children, triageStep } = props.item
  if (props.column === DONE_COLUMN) {
    const total = Math.max(children?.total ?? 0, DONE_SEGMENTS)
    return { total, filled: total, fillClass: 'bg-ok', showText: children !== undefined && children.total > 0 }
  }
  if (children) {
    return { total: children.total, filled: Math.min(children.done, children.total), fillClass: 'bg-ok', showText: true }
  }
  if (props.state === 'triagem') return { total: TRIAGE_STEP_COUNT, filled: triageStep ?? 0, fillClass: 'bg-accent', showText: false }
  return undefined
})

const percent = computed(() => {
  if (!progress.value || progress.value.total === 0) return 0
  return Math.round((progress.value.filled / progress.value.total) * 100)
})

const authorInitial = computed(() => (props.item.who === VOCE ? YOU_INITIAL : props.item.who.charAt(0).toUpperCase()))

const showPlan = computed(() => !isCompact.value && Boolean(props.planLine))

function tagPillStyle(tag: BacklogTag) {
  const token = colorVar(tagColor(tag))
  return {
    color: colorVar(tagTextColor(tag)),
    backgroundColor: `color-mix(in srgb, ${token} ${TAG_TINT_PERCENT}%, transparent)`,
  }
}
</script>

<template>
  <article
    class="flex cursor-pointer flex-col gap-2 rounded-card border border-line bg-surface p-3.5"
    :class="[isCompact && 'border-l-4', item.error && 'border-danger']"
    :style="isCompact ? { borderLeftColor: featureColor } : undefined"
    @click="emit('open')"
  >
    <header class="flex items-center gap-2 text-caption text-muted">
      <span>{{ item.id }}</span>
      <UiBadge v-if="item.type === 'feature'" status="backlog" label="feature" />
      <UiBadge v-if="item.isNew" status="lote" label="novo" />
      <ul v-if="!isCompact && item.tags.length" class="ml-auto flex min-w-0 flex-wrap justify-end gap-1.5">
        <li
          v-for="tag in item.tags"
          :key="tag"
          class="inline-flex h-5.5 items-center rounded-chip px-2 text-meta font-medium whitespace-nowrap"
          :style="tagPillStyle(tag)"
        >
          {{ tagLabel(tag) }}
        </li>
      </ul>
      <div v-if="isCompact" class="ml-auto flex items-center gap-1.5">
        <UiBadge :status="chip.status" :label="chip.label" />
        <ul v-if="item.tags.length" class="flex items-center gap-1.5">
          <li
            v-for="tag in item.tags"
            :key="tag"
            class="size-2 rounded-full"
            :style="{ backgroundColor: colorVar(tagColor(tag)) }"
            :title="tagLabel(tag)"
          />
        </ul>
      </div>
    </header>

    <p v-if="item.parent && !isCompact" class="text-caption text-muted">↳ {{ item.parent }}</p>
    <h3 class="text-control font-semibold text-ink" :class="isCompact ? 'truncate' : 'wrap-break-word'">
      {{ item.title }}
    </h3>
    <p v-if="item.error" role="alert" class="text-caption font-semibold text-danger">
      Erro ao ler este item: {{ item.error.reason }}
    </p>

    <div v-if="!isCompact && action" class="flex items-center gap-1">
      <UiButton variant="quiet" size="sm" class="gap-1.5" :title="ACTION_TITLE[action.id]" @click.stop @press="emit('action', action.id)">
        <svg v-if="action.id === 'triar'" width="12" height="12" viewBox="0 0 16 16" aria-hidden="true">
          <path d="M2 3h12l-4.5 5.5V13l-3 1.5V8.5z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
        </svg>
        {{ action.label }}
      </UiButton>
      <UiButton
        v-if="action.id === 'triar'"
        variant="quiet"
        size="sm"
        :aria-label="ACTION_TITLE.vincular"
        :title="ACTION_TITLE.vincular"
        @click.stop
        @press="emit('action', 'vincular')"
      >
        <svg width="13" height="13" viewBox="0 0 16 16" aria-hidden="true">
          <path
            d="M6.5 9.5l3-3M7 4.5l1-1a2.5 2.5 0 013.5 3.5l-1 1M9 11.5l-1 1A2.5 2.5 0 014.5 9l1-1"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
          />
        </svg>
      </UiButton>
    </div>

    <p v-if="!isCompact && item.description" class="text-caption text-muted">{{ item.description }}</p>
    <p v-if="showPlan" class="wrap-break-word whitespace-pre-wrap text-caption text-ink">{{ planLine }}</p>

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
      <UiBadge v-if="!isCompact" class="ml-auto" :status="chip.status" :label="chip.label" />
    </footer>
  </article>
</template>
