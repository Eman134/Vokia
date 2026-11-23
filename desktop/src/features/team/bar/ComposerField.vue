<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'
import { useConnection } from '../useConnection'
import { useFeed } from '../useFeed'
import { useFlow } from '../useFlow'
import { useRequests } from '../useRequests'
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
import AttachmentChips from '../../attachments/AttachmentChips.vue'
import type { useComposerAttachments } from '../../attachments/useComposerAttachments'
import UiButton from '../../../ui/UiButton.vue'

const ATTACH_LABEL = 'Anexar arquivo'

const props = defineProps<{
  canAsk: boolean
  interacting: boolean
  inlinePermission: boolean
  railPage: RailPage | null
  pill: HTMLElement | null
  attachments: ReturnType<typeof useComposerAttachments>
}>()

const emit = defineEmits<{
  'menu-open': [open: boolean]
}>()

// O ícone não pode ocupar largura no espelho, senão o texto desalinha do campo: sobre o @ ele cabe no
// lugar do caractere; sem @, fica acima da primeira letra.
const AT_ICON_SIZE = 12
const RAISED_ICON_SIZE = 9

// Deve casar com o pl-2 do campo e do espelho, para o menu cair sobre o @.
const FIELD_PADDING_X = 8
// Devem casar com o w-65 do MentionMenu e o w-75 do CommandMenu.
const MENU_WIDTH_BY_KIND = { mention: 260, command: 300 } as const

const connection = useConnection()
const feed = useFeed()
const requests = useRequests()
const roster = useRoster()
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
  if (draft.value.trim() === '') {
    return null
  }
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

// Só intercepta quando há arquivo (ex.: print); colar texto segue o caminho padrão do input.
function onPaste(event: ClipboardEvent): void {
  const files = Array.from(event.clipboardData?.files ?? [])
  if (files.length === 0) {
    return
  }
  event.preventDefault()
  void props.attachments.addFiles(files)
}

function onBackspace(event: KeyboardEvent): void {
  const input = inputRef.value
  if (!input || input.selectionStart !== 0 || input.selectionEnd !== 0) return
  const last = props.attachments.attachments.value.at(-1)
  if (!last) return
  event.preventDefault()
  props.attachments.remove(last.id)
}

function isFocused(): boolean {
  return document.activeElement === inputRef.value
}

defineExpose({ isFocused })
</script>

<template>
  <div
    v-if="canAsk && !composerHidden"
    class="flex h-8.5 min-w-28 flex-1 items-center rounded-full bg-surface-2 pl-1.5 focus-within:outline-2 focus-within:outline-solid focus-within:outline-accent"
  >
    <UiButton
      variant="quiet"
      size="sm"
      class="size-6.5 shrink-0 !px-0 text-muted hover:text-ink"
      :aria-label="ATTACH_LABEL"
      :title="ATTACH_LABEL"
      :disabled="attachments.saving.value || locked"
      @press="attachments.pickFiles()"
    >
      <svg viewBox="0 0 24 24" class="size-3.5 shrink-0" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="m21.4 11.1-9.2 9.2a6 6 0 0 1-8.5-8.5l9.2-9.2a4 4 0 0 1 5.7 5.7l-9.2 9.2a2 2 0 0 1-2.8-2.8l8.5-8.5" />
      </svg>
    </UiButton>
    <AttachmentChips
      :attachments="attachments.attachments.value"
      :saving="attachments.saving.value"
      class="max-w-[45%]"
      @remove="attachments.remove($event)"
    />
    <div
      ref="wrapperRef"
      class="relative h-full min-w-0 flex-1 overflow-hidden rounded-r-full"
    >
      <div
        aria-hidden="true"
        class="pointer-events-none absolute inset-y-0 left-0 flex items-center whitespace-pre pl-2 pr-3.5 text-label"
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
        class="relative h-full w-full bg-transparent pl-2 pr-3.5 text-label text-transparent caret-ink outline-none placeholder:text-muted selection:bg-accent/25 selection:text-transparent disabled:opacity-40"
        :placeholder="placeholder"
        :disabled="locked"
        @keydown="onKeydown"
        @keydown.backspace="onBackspace"
        @input="syncCaret"
        @click="syncCaret"
        @keyup="syncCaret"
        @select="syncCaret"
        @scroll="syncCaret"
        @blur="dismissMenu"
        @paste="onPaste"
      />
    </div>
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
