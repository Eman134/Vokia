<script setup lang="ts">
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiSketchBox from '../../ui/UiSketchBox.vue'
import UiButton from '../../ui/UiButton.vue'
import UiMarker from '../../ui/UiMarker.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import { useRoster } from '../team/useRoster'
import UserAvatar from '../profile/UserAvatar.vue'
import {
  ACTION_ID,
  ACTION_TITLE,
  type ActionLabel,
  BADGE_DOT,
  AUTO_CLOSED_LABEL,
  CLOSED_BY_AUTOMATIC,
  COLUMN_CHIP,
  STATE_ACTION,
  STATE_BADGE,
  STATE_CHIP,
  TRIAGE_STEP_COUNT,
} from './backlogLabels'
import type { ParsedProposal } from './planProposal'
import { tagLabel, tagMarkerTone } from './tags'
import {
  DEFAULT_COLUMN,
  isUnassigned,
  READY_COLUMN,
  UNASSIGNED_TEXT,
  type BacklogCardAction,
  type BacklogColumn,
  type BacklogItem,
  type BacklogState,
} from './backlogTypes'

import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
const props = defineProps<{
  item: BacklogItem
  column: BacklogColumn
  state: BacklogState
  planLine?: string
  proposal?: ParsedProposal
}>()
const emit = defineEmits<{ open: []; action: [action: BacklogCardAction] }>()

const DONE_SEGMENTS = 1
const DONE_COLUMN: BacklogColumn = 'Done'
const AVATAR_SIZE = 22
const KEY_POINTS_LIMIT = 3
const VISIBLE_TAGS_LIMIT = 2
const TAG_CLASSES = 'inline-block max-w-24 truncate align-bottom text-meta whitespace-nowrap'
const CARD_CLASSES =
  'relative flex min-w-0 cursor-pointer flex-col gap-2 rounded-sm border border-line bg-surface px-3.25 py-3'
// Proposta é um post-it dentro da ficha: o texto escuro vale nos dois temas porque o amarelo é pastel nos dois.
const PROPOSAL_CLASSES = 'flex flex-col gap-1.5 rounded-sm bg-sticky px-2.5 py-2 text-mark-ink shadow-seg'

const roster = useRoster()

const chip = computed(
  () => COLUMN_CHIP[props.column] ?? { label: STATE_CHIP[props.state], status: STATE_BADGE[props.state] },
)
const visibleTags = computed(() => props.item.tags.slice(0, VISIBLE_TAGS_LIMIT))
const hiddenTags = computed(() => props.item.tags.slice(VISIBLE_TAGS_LIMIT))
const isFeature = computed(() => props.item.type === 'feature')
const isAutoClosed = computed(() => props.column === DONE_COLUMN && props.item.closedBy === CLOSED_BY_AUTOMATIC)
const ownerName = computed(() => (isUnassigned(props.item) ? UNASSIGNED_TEXT : (roster.profileOf(props.item.who)?.name ?? props.item.who)))
const createdOn = computed(() => props.item.activity?.[0]?.at.split(',')[0])

const action = computed<{ label: ActionLabel; id: BacklogCardAction } | undefined>(() => {
  if (props.item.error) return undefined
  if (props.column === READY_COLUMN && props.state === 'vinculado') return { label: 'Iniciar', id: 'iniciar' }
  if (props.column !== DEFAULT_COLUMN) return undefined
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

</script>

<template>
  <article
    :class="CARD_CLASSES"
    @click="emit('open')"
  >
    <UiSketchBox v-if="item.error" tone="danger" class="pointer-events-none absolute inset-0" />
    <header class="flex items-center gap-2">
      <span class="font-mono text-tiny text-muted">{{ item.id }}</span>
      <UiMarker v-if="isFeature" tone="blue" class="text-tiny font-semibold">feature</UiMarker>
      <UiBadge v-if="item.isNew" status="lote" label="novo" />
      <UiBadge v-if="isAutoClosed" status="trabalhando" :label="AUTO_CLOSED_LABEL" />
      <ul v-if="item.tags.length" class="ml-auto flex min-w-0 flex-nowrap items-center justify-end gap-1">
        <li v-for="tag in visibleTags" :key="tag" class="min-w-0">
          <UiMarker :tone="tagMarkerTone(tag)" :class="TAG_CLASSES">{{ tagLabel(tag) }}</UiMarker>
        </li>
        <li v-if="hiddenTags.length" class="shrink-0 text-meta text-muted" :title="hiddenTags.map(tagLabel).join(', ')">+{{ hiddenTags.length }}</li>
      </ul>
    </header>

    <p v-if="item.parent" class="truncate text-meta text-muted">↳ {{ item.parent }}</p>
    <h3 class="flex items-start gap-2 text-body leading-[1.3] font-bold text-ink">
      <span class="mt-1.25 size-2 shrink-0 rounded-full" :class="BADGE_DOT[chip.status]" :title="chip.label" aria-hidden="true" />
      <span class="sr-only">{{ chip.label }}:</span>
      <span class="min-w-0 wrap-break-word">{{ item.title }}</span>
    </h3>
    <p v-if="item.description" class="line-clamp-2 wrap-break-word text-caption leading-[1.45] text-muted">{{ item.description }}</p>
    <p v-if="item.error" role="alert" class="text-caption font-semibold text-danger">
      Erro ao ler este item: {{ item.error.reason }}
    </p>
    <p v-if="planLine" class="line-clamp-2 wrap-break-word whitespace-pre-wrap text-caption text-ink">{{ planLine }}</p>
    <div v-if="proposal" :class="PROPOSAL_CLASSES">
      <p v-if="proposal.summary" class="line-clamp-2 wrap-break-word text-caption leading-[1.45]">{{ proposal.summary }}</p>
      <p class="text-meta">{{ proposal.lots.length }} etapas<template v-if="proposal.estimate"> · {{ proposal.estimate }}</template></p>
      <ul v-if="proposal.lots.length" class="flex list-disc flex-col gap-0.5 pl-3.5 text-meta">
        <li v-for="lot in proposal.lots.slice(0, KEY_POINTS_LIMIT)" :key="lot.n" class="truncate">{{ lot.title }}</li>
      </ul>
    </div>

    <div v-if="action" class="flex items-center gap-1">
      <UiButton variant="primary" size="sm" class="h-6.5 gap-1.5 rounded-sm pr-2.25 pl-1.75 text-small font-semibold" :title="ACTION_TITLE[action.id]" @click.stop @press="emit('action', action.id)">
        <svg v-if="action.id === 'triar'" width="12" height="12" viewBox="0 0 16 16" aria-hidden="true">
          <path d="M2 3h12l-4.5 5.5V13l-3 1.5V8.5z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
        </svg>
        {{ action.label }}
      </UiButton>
      <button
        v-if="action.id === 'triar'"
        type="button"
        class="inline-flex size-6.5 shrink-0 cursor-pointer items-center justify-center rounded-sm border border-line text-muted hover:bg-hover hover:text-ink" :class="INK_FOCUS_CLASSES"
        :aria-label="ACTION_TITLE.vincular"
        :title="ACTION_TITLE.vincular"
        @click.stop="emit('action', 'vincular')"
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
      </button>
    </div>

    <div v-if="progress" class="flex flex-col gap-1.25">
      <p v-if="progress.showText" class="flex justify-between text-meta text-muted">
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

    <footer class="flex items-center gap-2 pt-0.5">
      <UserAvatar v-if="isUnassigned(item)" :size="AVATAR_SIZE" />
      <AgentPortrait v-else :who="item.who" :size="AVATAR_SIZE" />
      <span class="text-meta text-muted">{{ ownerName }}<template v-if="createdOn"> · {{ createdOn }}</template></span>
    </footer>
  </article>
</template>
