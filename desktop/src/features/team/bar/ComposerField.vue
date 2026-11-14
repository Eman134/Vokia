<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'
import { useConnection } from '../useConnection'
import { useFeed } from '../useFeed'
import { useFlow } from '../useFlow'
import { useRequests } from '../useRequests'
import { useRoster } from '../useRoster'
import { useTeamStatus } from '../useTeamStatus'
import { placeholderFor, QUEUE_SUFFIX } from '../composerPlaceholder'
import { mentionSegments, mentionSpans } from '../composerMentions'
import { FLOW_PLACEHOLDER } from '../flowSteps'
import type { Agent } from '../events'
import type { RailPage } from '../../../shell/railNav'
import { useProjects } from '../../projects/useProjects'
import MentionMenu from '../MentionMenu.vue'
import CommandMenu from '../CommandMenu.vue'
import { useComposerMenus } from './useComposerMenus'

const props = defineProps<{
  canAsk: boolean
  interacting: boolean
  inlinePermission: boolean
  railPage: RailPage | null
  pill: HTMLElement | null
}>()

const emit = defineEmits<{
  'menu-open': [open: boolean]
}>()

// Deve casar com o px-3.5 do campo e do espelho, para o menu cair sobre o @.
const FIELD_PADDING_X = 14
// Devem casar com o w-65 do MentionMenu e o w-75 do CommandMenu.
const MENU_WIDTH_BY_KIND = { mention: 260, command: 300 } as const

const connection = useConnection()
const feed = useFeed()
const requests = useRequests()
const roster = useRoster()
const statusStore = useTeamStatus()
const flow = useFlow()
const projectsStore = useProjects()
const draft = defineModel<string>('draft', { required: true })
const confirmed = defineModel<Set<Agent>>('confirmed', { required: true })
const caret = ref(0)
const scrollLeft = ref(0)
const menuLeft = ref(0)
const inputRef = ref<HTMLInputElement | null>(null)
const wrapperRef = ref<HTMLElement | null>(null)
const measureRef = ref<HTMLElement | null>(null)

const locked = computed(() => connection.checking)
const {
  menuKind,
  activeMatch,
  mentionMatches,
  commandMatches,
  menuCount,
  highlightedIndex,
  menuOpen,
  activeOptionId,
  listboxId,
  chooseMentionAgent,
  chooseCommand,
  insertAtCaret,
  dismissMenu,
  onKeydown,
} = useComposerMenus({ draft, caret, confirmed, locked, placeCaret })
const placeholder = computed(() =>
  flow.flow !== null
    ? FLOW_PLACEHOLDER[flow.flow]
    : placeholderFor(props.railPage, projectsStore.selected?.name ?? '', statusStore.status, statusStore.placeholder),
)
const spans = computed(() => mentionSpans(draft.value, confirmed.value))
const recipient = computed<Agent | null>(() => spans.value[0]?.agent ?? null)
const segments = computed(() => mentionSegments(draft.value, spans.value))
const queueHint = computed(() => {
  const target = recipient.value
  if (target === null || feed.stateOf(target) === 'idle') {
    return null
  }
  const onlyMentions = segments.value.every((segment) => segment.agent !== null || segment.text.trim() === '')
  return onlyMentions ? QUEUE_SUFFIX : null
})

// O campo é desmontado offline; sem o false final o pai ficaria com o menu aberto sem menu visível.
watch(menuOpen, (open) => emit('menu-open', open), { immediate: true })
onUnmounted(() => emit('menu-open', false))

watch(draft, (text) => {
  const present = new Set(mentionSpans(text, confirmed.value).map((span) => span.agent))
  if (present.size !== confirmed.value.size) {
    confirmed.value = present
  }
})

watch([activeMatch, scrollLeft], async () => {
  await nextTick()
  menuLeft.value = anchorLeft()
})

watch(
  () => requests.pendingMention,
  () => {
    const requestedId = requests.takePendingMention()
    const requested = roster.agents.find((agent) => agent.id === requestedId)
    if (requested !== undefined) {
      insertAtCaret(requested)
    }
  },
)

function anchorLeft(): number {
  const wrapper = wrapperRef.value
  const measure = measureRef.value
  const pill = props.pill
  if (wrapper === null || measure === null || pill === null) {
    return 0
  }
  const atX = wrapper.offsetLeft + FIELD_PADDING_X + measure.offsetWidth - scrollLeft.value
  const width = MENU_WIDTH_BY_KIND[menuKind.value ?? 'mention']
  return Math.max(0, Math.min(atX, pill.clientWidth - width))
}

function syncCaret(): void {
  caret.value = inputRef.value?.selectionStart ?? draft.value.length
  scrollLeft.value = inputRef.value?.scrollLeft ?? 0
}

async function placeCaret(position: number): Promise<void> {
  await nextTick()
  inputRef.value?.focus()
  inputRef.value?.setSelectionRange(position, position)
  syncCaret()
}

function isFocused(): boolean {
  return document.activeElement === inputRef.value
}

defineExpose({ isFocused })
</script>

<template>
  <div
    v-if="canAsk"
    ref="wrapperRef"
    class="relative h-8.5 min-w-0 flex-1 overflow-hidden rounded-full bg-surface-2"
    :class="{ 'sr-only': !interacting || (inlinePermission && draft === '') }"
  >
    <div
      aria-hidden="true"
      class="pointer-events-none absolute inset-y-0 left-0 flex items-center whitespace-pre px-3.5 text-label"
      :style="{ transform: `translateX(${-scrollLeft}px)` }"
    >
      <span
        v-for="(segment, index) in segments"
        :key="index"
        :class="segment.agent === null ? 'text-ink' : 'text-accent'"
        >{{ segment.text }}</span
      >
      <span v-if="queueHint" class="text-muted">{{ queueHint }}</span>
      <span ref="measureRef" class="invisible absolute whitespace-pre">{{
        draft.slice(0, activeMatch?.start ?? 0)
      }}</span>
    </div>
    <input
      ref="inputRef"
      v-model="draft"
      role="combobox"
      aria-autocomplete="list"
      :aria-expanded="menuOpen"
      :aria-controls="menuOpen && menuCount > 0 ? listboxId : undefined"
      :aria-activedescendant="activeOptionId"
      class="relative h-full w-full bg-transparent px-3.5 text-label text-transparent caret-ink outline-none placeholder:text-muted selection:bg-accent-soft focus:outline-2 focus:outline-solid focus:outline-accent disabled:opacity-40"
      :placeholder="placeholder"
      :disabled="locked"
      @keydown="onKeydown"
      @input="syncCaret"
      @click="syncCaret"
      @keyup="syncCaret"
      @select="syncCaret"
      @scroll="syncCaret"
      @blur="dismissMenu"
    />
  </div>
  <CommandMenu
    v-if="menuOpen && menuKind === 'command'"
    :id="listboxId"
    :commands="commandMatches"
    :active-index="highlightedIndex"
    :left="menuLeft"
    @choose="chooseCommand"
  />
  <MentionMenu
    v-else-if="menuOpen"
    :id="listboxId"
    :agents="mentionMatches"
    :active-index="highlightedIndex"
    :left="menuLeft"
    @choose="chooseMentionAgent"
  />
</template>
