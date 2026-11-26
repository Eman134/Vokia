<script setup lang="ts">
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiSketchBox from '../../ui/UiSketchBox.vue'
import UiMarker from '../../ui/UiMarker.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import { useRoster } from '../team/useRoster'
import { formatItemUsage } from '../team/itemUsage'
import { useItemUsage } from '../team/useItemUsage'
import UserAvatar from '../profile/UserAvatar.vue'
import {
  BADGE_DOT,
  AUTO_CLOSED_LABEL,
  CLOSED_BY_AUTOMATIC,
  COLUMN_CHIP,
  STATE_BADGE,
  STATE_CHIP,
  TRIAGE_STEP_COUNT,
} from './backlogLabels'
import type { ParsedProposal } from './planProposal'
import {
  isUnassigned,
  UNASSIGNED_TEXT,
  type TaskCardAction,
  type BacklogColumn,
  type BacklogItem,
  type BacklogState,
} from './backlogTypes'
import { cardActionFor } from './cardAction'
import BacklogPlanActions from './BacklogPlanActions.vue'
import { extractAttachments } from '../attachments/attachmentBlock'
import AttachmentPreviewList from '../attachments/AttachmentPreviewList.vue'

const props = defineProps<{
  item: BacklogItem
  column: BacklogColumn
  state: BacklogState
  planLine?: string
  proposal?: ParsedProposal
}>()
const emit = defineEmits<{ open: []; action: [action: TaskCardAction] }>()

const DONE_SEGMENTS = 1
const DONE_COLUMN: BacklogColumn = 'Done'
const AVATAR_SIZE = 22
const MAX_VISIBLE_TAGS = 3
const CARD_CLASSES =
  'relative flex min-w-0 cursor-pointer flex-col gap-1 rounded-sm border border-line bg-surface px-2 py-2 lg:gap-1.5 lg:px-3 lg:py-2.5'
const ID_CLASSES = 'shrink-0 rounded-sm bg-surface-2 px-1.5 py-0.5 font-mono text-tiny font-bold tracking-wide text-ink'
const TAG_CLASSES = 'max-w-[110px] truncate rounded-sm border border-line bg-surface-2 px-1.5 py-0.5 text-tiny text-muted'

const roster = useRoster()
const itemUsage = useItemUsage(() => props.item.id)
const usageText = computed(() => formatItemUsage(itemUsage.value))
const displayId = computed(() => props.item.publicId ?? props.item.id)
const visibleTags = computed(() => props.item.tags.slice(0, MAX_VISIBLE_TAGS))
const extraTagsCount = computed(() => Math.max(0, props.item.tags.length - MAX_VISIBLE_TAGS))
const descriptionContent = computed(() => extractAttachments(props.item.description ?? ''))

const chip = computed(
  () => COLUMN_CHIP[props.column] ?? { label: STATE_CHIP[props.state], status: STATE_BADGE[props.state] },
)
const isFeature = computed(() => props.item.type === 'feature')
const isAutoClosed = computed(() => props.column === DONE_COLUMN && props.item.closedBy === CLOSED_BY_AUTOMATIC)
const ownerName = computed(() => (isUnassigned(props.item) ? UNASSIGNED_TEXT : (roster.profileOf(props.item.who)?.name ?? props.item.who)))

const action = computed(() => cardActionFor(props.item, props.column, props.state))

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

</script>

<template>
  <article
    :class="CARD_CLASSES"
    @click="emit('open')"
  >
    <UiSketchBox v-if="item.error" tone="danger" class="pointer-events-none absolute inset-0" />
    <header class="flex flex-wrap items-center gap-1.5">
      <span class="size-2 shrink-0 rounded-full" :class="BADGE_DOT[chip.status]" :title="chip.label" aria-hidden="true" />
      <span class="sr-only">{{ chip.label }}:</span>
      <span :class="ID_CLASSES" data-testid="card-id">{{ displayId }}</span>
      <UiMarker v-if="isFeature" tone="blue" class="text-tiny font-semibold">feature</UiMarker>
      <UiBadge v-if="isAutoClosed" status="trabalhando" :label="AUTO_CLOSED_LABEL" />
    </header>

    <p v-if="item.parent" class="hidden truncate text-meta text-muted lg:block">↳ {{ item.parent }}</p>
    <h3 class="min-w-0 line-clamp-2 text-caption lg:text-body leading-[1.3] font-bold text-ink" :title="item.title">
      {{ item.title }}
    </h3>
    <div v-if="visibleTags.length > 0" class="hidden flex-wrap items-center gap-1 lg:flex" data-testid="card-tags">
      <span v-for="tag in visibleTags" :key="tag" :class="TAG_CLASSES" :title="tag">{{ tag }}</span>
      <span v-if="extraTagsCount > 0" class="text-tiny text-muted">+{{ extraTagsCount }}</span>
    </div>
    <p v-if="usageText" class="hidden text-meta text-muted xl:block" data-testid="card-usage">{{ usageText }}</p>
    <AttachmentPreviewList
      v-if="descriptionContent.attachments.length > 0"
      :attachments="descriptionContent.attachments"
      compact
      class="hidden lg:flex"
      data-testid="card-attachments"
    />
    <p v-if="item.error" role="alert" class="text-caption font-semibold text-danger">
      Erro ao ler este item: {{ item.error.reason }}
    </p>

    <div class="mt-auto flex items-center gap-1.5 pt-0.5 lg:gap-2">
      <BacklogPlanActions v-if="action" :action="action" class="hidden lg:flex" @action="emit('action', $event)" />
      <span class="ml-auto shrink-0" :title="ownerName">
        <UserAvatar v-if="isUnassigned(item)" :size="AVATAR_SIZE" />
        <AgentPortrait v-else :who="item.who" :size="AVATAR_SIZE" />
      </span>
    </div>

    <div v-if="progress" class="flex flex-col gap-1">
      <p v-if="progress.showText" class="hidden justify-between text-meta text-muted xl:flex">
        <span>{{ item.children?.total }} micro-tarefas · {{ progress.filled }} feitas</span>
        <span>{{ percent }}%</span>
      </p>
      <div class="flex gap-0.75">
        <span
          v-for="segment in progress.total"
          :key="segment"
          class="h-1 flex-1 rounded-sm"
          :class="segment <= progress.filled ? progress.fillClass : 'bg-surface-2'"
        />
      </div>
    </div>
  </article>
</template>
