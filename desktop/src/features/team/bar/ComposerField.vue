<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'
import { useConnection } from '../useConnection'
import { useFeed } from '../useFeed'
import { useFlow } from '../useFlow'
import { useRequests } from '../useRequests'
import { useMentionCard } from '../useMentionCard'
import { agentName } from '../agents'
import { useRoster } from '../useRoster'
import { useTeamStatus } from '../useTeamStatus'
import { placeholderFor, QUEUE_SUFFIX } from '../composerPlaceholder'
import { mentionSegments, mentionSpans, MENTION_TRIGGER } from '../composerMentions'
import { findAgentMentions } from '../agentMentions'
import AgentPortrait from '../AgentPortrait.vue'
import { FLOW_PLACEHOLDER } from '../flowSteps'
import { screenDestination, screenPlaceholder } from '../screenRecipient'
import { useChannels } from '../../channels/useChannels'
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

// O ícone não pode ocupar largura no espelho, senão o texto desalinha do campo: sobre o @ ele cabe no
// lugar do caractere; sem @, fica acima da primeira letra.
const AT_ICON_SIZE = 12
const RAISED_ICON_SIZE = 9

// Deve casar com o px-3.5 do campo e do espelho, para o menu cair sobre o @.
const FIELD_PADDING_X = 14
// Devem casar com o w-65 do MentionMenu e o w-75 do CommandMenu.
const MENU_WIDTH_BY_KIND = { mention: 260, command: 300 } as const

const connection = useConnection()
const feed = useFeed()
const requests = useRequests()
const roster = useRoster()
const mentionCard = useMentionCard()
const channelsStore = useChannels()
const rosterOrder = computed(() => roster.agents.map((profile) => profile.id))
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
const composerHidden = computed(() => !props.interacting || (props.inlinePermission && draft.value === ''))
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
    : placeholderFor(
        props.railPage,
        projectsStore.selected?.name ?? '',
        statusStore.status,
        statusStore.placeholder,
        screenPlaceholder(props.railPage, feed.screen, agentName, channelsStore.channels, rosterOrder.value),
      ),
)
const spans = computed(() => mentionSpans(draft.value, confirmed.value))
const recipient = computed<Agent | null>(
  () =>
    screenDestination(props.railPage, feed.screen, channelsStore.channels, rosterOrder.value, spans.value[0]?.agent ?? null)
      .to,
)
const segments = computed(() => mentionSegments(draft.value, spans.value))
// Só o destaque visual: o destino do pedido continua vindo das menções confirmadas (spans).
const highlightSegments = computed(() =>
  mentionSegments(
    draft.value,
    findAgentMentions(draft.value, roster.agents).map(({ start, end, agent }) => ({ start, end, agent: agent.id })),
  ),
)
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

// O espelho não recebe o ponteiro (o campo fica por cima), então o hover é achado pela posição no eixo x.
function hoverMention(event: MouseEvent): void {
  const mentions = wrapperRef.value?.querySelectorAll<HTMLElement>('[data-mention]') ?? []
  for (const element of mentions) {
    const { left, right } = element.getBoundingClientRect()
    if (event.clientX >= left && event.clientX <= right && element.dataset.mention) {
      mentionCard.open(element.dataset.mention, element)
      return
    }
  }
  mentionCard.scheduleClose()
}

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
    v-if="canAsk && !composerHidden"
    ref="wrapperRef"
    class="relative h-8.5 min-w-0 flex-1 overflow-hidden rounded-full bg-surface-2"
    @mousemove="hoverMention"
    @mouseleave="mentionCard.scheduleClose()"
  >
    <div
      aria-hidden="true"
      class="pointer-events-none absolute inset-y-0 left-0 flex items-center whitespace-pre px-3.5 text-label"
      :style="{ transform: `translateX(${-scrollLeft}px)` }"
    >
      <template v-for="(segment, index) in highlightSegments" :key="index">
        <span v-if="segment.agent === null" class="text-ink">{{ segment.text }}</span>
        <span v-else :data-mention="segment.agent" class="relative text-accent">
          <span
            v-if="segment.text.startsWith(MENTION_TRIGGER)"
            class="absolute top-1/2 left-0 -translate-y-1/2"
            ><AgentPortrait :who="segment.agent" :size="AT_ICON_SIZE"
          /></span>
          <span v-else class="absolute bottom-full left-0"
            ><AgentPortrait :who="segment.agent" :size="RAISED_ICON_SIZE"
          /></span>
          <span :class="segment.text.startsWith(MENTION_TRIGGER) ? 'text-transparent' : ''">{{
            segment.text.startsWith(MENTION_TRIGGER) ? MENTION_TRIGGER : ''
          }}</span
          >{{ segment.text.startsWith(MENTION_TRIGGER) ? segment.text.slice(MENTION_TRIGGER.length) : segment.text }}
        </span>
      </template>
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
      class="relative h-full w-full bg-transparent px-3.5 text-label text-transparent caret-ink outline-none placeholder:text-muted selection:bg-accent/25 selection:text-transparent focus:outline-2 focus:outline-solid focus:outline-accent disabled:opacity-40"
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
