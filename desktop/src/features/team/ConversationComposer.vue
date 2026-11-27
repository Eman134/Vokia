<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import { useNavigation } from '../../shell/useNavigation'
import AgentPicker from '../agents/AgentPicker.vue'
import AgentPortrait from './AgentPortrait.vue'
import AttachmentChips from '../attachments/AttachmentChips.vue'
import CommandMenu from './CommandMenu.vue'
import MentionMenu from './MentionMenu.vue'
import { useBacklog } from '../backlog/useBacklog'
import { useTriage } from '../backlog/useTriage'
import { useRunItem } from '../backlog/useRunItem'
import { isUnassigned } from '../backlog/backlogTypes'
import { useChannels } from '../channels/useChannels'
import { agentName } from './agents'
import { findAgentMentions } from './agentMentions'
import { useComposerMenus } from './bar/useComposerMenus'
import { useComposerSubmit } from './bar/useComposerSubmit'
import { MENTION_TRIGGER, mentionSegments, mentionSpans } from './composerMentions'
import type { Agent } from './events'
import { useThreads } from './useThreads'
import { screenPlaceholder } from './screenRecipient'
import { COMMAND_TRIGGER, SLASH_COMMANDS } from './slashCommands'
import { useConnection } from './useConnection'
import { useFeed } from './useFeed'
import { useRequests } from './useRequests'
import { useRoster } from './useRoster'

const FALLBACK_PLACEHOLDER = 'Mensagem para o time'
const FIELD_LABEL = 'Mensagem rápida'
const SEND_LABEL = 'Enviar'
const ATTACH_LABEL = 'Anexar arquivo'
const COMMANDS_LABEL = 'Comandos'
const FOOTER_CLASSES = 'flex shrink-0 flex-col gap-1.5 border-t border-line bg-paper px-4 py-2.5'
// Mesma pílula da ActionBar: o clipe e o @ moram dentro dela, à esquerda, e o campo não tem borda própria.
const PILL_CLASSES = 'flex min-h-8.5 min-w-0 flex-1 items-center rounded-full bg-surface-2 pl-1.5 focus-within:outline-2 focus-within:outline-solid focus-within:outline-ink'
const FIELD_TEXT_CLASSES = 'py-1.5 pl-2 pr-3.5 font-sans text-label leading-5 [overflow-wrap:anywhere]'
const FIELD_CLASSES = `relative block field-sizing-content max-h-28 min-h-8 w-full min-w-0 resize-none border-0 bg-transparent ${FIELD_TEXT_CLASSES} text-transparent caret-ink outline-none placeholder:text-muted selection:bg-accent/25 selection:text-transparent`
const AT_ICON_SIZE = 12
const RAISED_ICON_SIZE = 9
const ICON_SIZE_CLASSES = 'size-4 shrink-0'
const ATTACH_CLASSES = 'size-6.5 shrink-0 px-0! text-muted hover:text-ink'
const SEND_CLASSES = 'size-9 shrink-0 rounded-full px-0!'
const CHIP_CLASSES = `cursor-pointer rounded-sm border border-line px-2 py-0.5 font-mono text-meta text-muted hover:bg-hover hover:text-ink ${INK_FOCUS_CLASSES}`

// Só o que tem texto pronto para o agente: /pausar e /lembrar ainda não existem e /setup não é de conversa.
const chips = SLASH_COMMANDS.filter((command) => command.enabled && command.id !== 'setup')

const connection = useConnection()
const feed = useFeed()
const requests = useRequests()
const roster = useRoster()
const backlog = useBacklog()
const triage = useTriage()
const channels = useChannels()
const threads = useThreads()
const navigation = useNavigation()
const { draft, confirmed, submit, attachments } = useComposerSubmit('conversation')
const field = ref<HTMLTextAreaElement>()
const caret = ref(0)
const agentPickerOpen = ref(false)
const locked = computed(() => connection.checking)

const {
  menuKind,
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

const spans = computed(() => mentionSpans(draft.value, confirmed.value))
const highlightSegments = computed(() =>
  mentionSegments(
    draft.value,
    findAgentMentions(draft.value, roster.agents).map(({ start, end, agent }) => ({ start, end, agent: agent.id })),
  ),
)
const scrollTop = ref(0)
const scrollLeft = ref(0)
const selectedMentionRecipient = computed<Agent | null>(() => spans.value[0]?.agent ?? null)
const queuedCounts = computed<Record<string, number>>(() => {
  const counts: Record<string, number> = {}
  for (const entry of requests.queue) {
    counts[entry.to] = (counts[entry.to] ?? 0) + 1
  }
  return counts
})
const recipientTooltip = computed(() =>
  selectedMentionRecipient.value
    ? `Destinatário: ${roster.nameOf(selectedMentionRecipient.value)} (trocar agente)`
    : 'Escolher agente destinatário',
)

function queueCountLabel(count: number): string {
  return `${count} na fila`
}

watch(draft, (text) => {
  const present = new Set(
    findAgentMentions(text, roster.agents).map((mention) => mention.agent.id),
  )
  const sameSize = present.size === confirmed.value.size
  const sameAll = sameSize && [...present].every((id) => confirmed.value.has(id))
  if (!sameAll) {
    confirmed.value = present
  }
})

function syncCaret(): void {
  caret.value = field.value?.selectionStart ?? draft.value.length
  scrollTop.value = field.value?.scrollTop ?? 0
  scrollLeft.value = field.value?.scrollLeft ?? 0
}

async function placeCaret(position: number): Promise<void> {
  await nextTick()
  field.value?.focus()
  field.value?.setSelectionRange(position, position)
  syncCaret()
}

function selectRecipient(agentId: Agent): void {
  const target = roster.agents.find((agent) => agent.id === agentId)
  if (target === undefined) return
  const firstSpan = spans.value[0]
  if (firstSpan !== undefined) {
    const replacement = `${MENTION_TRIGGER}${target.name}`
    const nextText = `${draft.value.slice(0, firstSpan.start)}${replacement}${draft.value.slice(firstSpan.end)}`
    const nextConfirmed = new Set(confirmed.value)
    nextConfirmed.delete(firstSpan.agent)
    nextConfirmed.add(target.id)
    draft.value = nextText
    confirmed.value = nextConfirmed
    const endPos = firstSpan.start + replacement.length
    const caretPos = nextText.slice(endPos).startsWith(' ') ? endPos + 1 : endPos
    void placeCaret(caretPos)
    return
  }
  insertAtCaret(target)
}

function onPickRecipient(agentId: Agent | null): void {
  if (!agentId) return
  void nextTick(() => selectRecipient(agentId))
}

const triageItems = computed(() => backlog.items.filter((item) => backlog.triagingIds.has(item.id)))
const selectedTriageItem = computed(() => {
  if (feed.screen.kind !== 'triage') return null
  return (
    triageItems.value.find((item) => item.id === triage.selectedTriageId) ??
    triageItems.value[0] ??
    null
  )
})
const triageTarget = computed(() => {
  const item = selectedTriageItem.value
  if (!item) return null
  return {
    itemId: item.id,
    agent: isUnassigned(item) ? null : item.who,
    label: item.publicId ?? item.id,
  }
})

const rosterOrder = computed(() => roster.agents.map((profile) => profile.id))
const activeThread = computed(() => (feed.screen.kind === 'thread' ? threads.find(feed.screen.itemId) : undefined))
const taskCommandMatch = computed(() => {
  const m = /^\/(triar|iniciar)(?:\s+(.*))?$/i.exec(draft.value.trimStart())
  if (!m) return null
  const cmd = m[1].toLowerCase() as 'triar' | 'iniciar'
  const query = (m[2] ?? '').trim().toLowerCase()
  const targetColumn = cmd === 'triar' ? 'Backlog' : 'Ready'
  const items = backlog.items.filter(
    (item) =>
      item.column === targetColumn &&
      (query === '' || item.title.toLowerCase().includes(query) || item.id.toLowerCase().includes(query)),
  )
  return { cmd, items }
})

async function selectTaskForCommand(itemId: string): Promise<void> {
  const cmd = taskCommandMatch.value?.cmd
  draft.value = ''
  const item = backlog.items.find((i) => i.id === itemId)
  if (!item) return
  if (cmd === 'triar') {
    if (isUnassigned(item)) {
      await useTriage().requestTriage(item)
      feed.notify(`Triagem iniciada para ${item.id} (${item.title}).`)
      return
    }
  } else if (cmd === 'iniciar') {
    const runItem = useRunItem()
    if (runItem.canRun(item.id)) {
      await runItem.run(item.id)
      feed.notify(`Execução iniciada para ${item.id} (${item.title}).`)
      return
    }
  }
  backlog.open(itemId)
}

const placeholder = computed(
  () =>
    screenPlaceholder(
      navigation.page,
      feed.screen,
      agentName,
      channels.channels,
      rosterOrder.value,
      activeThread.value?.title,
      triageTarget.value,
    ) ?? FALLBACK_PLACEHOLDER,
)
const hasContent = computed(() => draft.value.trim() !== '' || attachments.attachments.value.length > 0)
const canSend = computed(() => hasContent.value && !connection.checking && !attachments.saving.value)

function onFieldKeydown(event: KeyboardEvent): void {
  if (menuOpen.value && !taskCommandMatch.value) {
    onKeydown(event)
    if (event.defaultPrevented) return
  }
  if (event.key === 'Enter') {
    onEnter(event)
  }
}

// Enter envia, Shift+Enter quebra a linha; durante a composição de acento o Enter confirma a letra, não envia.
function onEnter(event: KeyboardEvent): void {
  if (event.isComposing || event.shiftKey) return
  event.preventDefault()
  void submit()
}

function onBackspace(event: KeyboardEvent): void {
  const input = field.value
  if (!input || input.selectionStart !== 0 || input.selectionEnd !== 0) return
  const last = attachments.attachments.value.at(-1)
  if (!last) return
  event.preventDefault()
  attachments.remove(last.id)
}

// Só intercepta quando há arquivo (ex.: print); colar texto segue o caminho padrão do campo.
function onPaste(event: ClipboardEvent): void {
  const files = Array.from(event.clipboardData?.files ?? [])
  if (files.length === 0) return
  event.preventDefault()
  void attachments.addFiles(files)
}

async function useChip(id: string): Promise<void> {
  draft.value = `${COMMAND_TRIGGER}${id} `
  await placeCaret(draft.value.length)
}
</script>

<template>
  <div :class="FOOTER_CLASSES" data-testid="conversation-composer">
    <div role="group" :aria-label="COMMANDS_LABEL" class="flex flex-wrap items-center gap-1.5">
      <button
        v-for="command in chips"
        :key="command.id"
        type="button"
        :class="CHIP_CLASSES"
        :title="command.description"
        @click="useChip(command.id)"
      >
        {{ COMMAND_TRIGGER }}{{ command.id }}
      </button>
    </div>
    <div v-if="feed.screen.kind === 'triage' && triageItems.length > 0" class="flex items-center gap-2 text-tiny text-muted">
      <span class="shrink-0 font-medium">Triagem:</span>
      <select
        :value="selectedTriageItem?.id ?? ''"
        class="h-6 max-w-xs cursor-pointer rounded-sm border border-line bg-surface px-1.5 text-tiny text-ink focus:border-ink focus:outline-none"
        aria-label="Selecionar triagem"
        data-testid="triage-target-select"
        @change="triage.selectedTriageId = ($event.target as HTMLSelectElement).value"
      >
        <option v-for="item in triageItems" :key="item.id" :value="item.id">
          {{ item.publicId ?? item.id }} · {{ item.title }}
        </option>
      </select>
    </div>
    <div class="relative flex items-end gap-2">
      <MentionMenu
        v-if="menuOpen && menuKind === 'mention'"
        :id="listboxId"
        :agents="mentionMatches"
        :active-index="highlightedIndex"
        :left="0"
        class="bottom-full mb-1.5"
        @choose="chooseMentionAgent"
      />
      <CommandMenu
        v-else-if="!taskCommandMatch && menuOpen && menuKind === 'command'"
        :id="listboxId"
        :commands="commandMatches"
        :active-index="highlightedIndex"
        :left="0"
        class="bottom-full mb-1.5"
        @choose="chooseCommand"
      />
      <ul
        v-if="taskCommandMatch && taskCommandMatch.items.length > 0"
        role="listbox"
        aria-label="Selecionar task"
        class="absolute bottom-full left-0 z-30 mb-1.5 max-h-56 w-80 max-w-full overflow-y-auto rounded-md border border-line bg-surface p-1 shadow-paper"
      >
        <li v-for="item in taskCommandMatch.items" :key="item.id">
          <button
            type="button"
            class="flex w-full cursor-pointer items-center gap-2 rounded-sm px-2.5 py-1.5 text-left text-caption text-ink hover:bg-hover"
            @click="selectTaskForCommand(item.id)"
          >
            <span class="shrink-0 font-mono text-meta text-muted">{{ item.id }}</span>
            <span class="min-w-0 truncate">{{ item.title }}</span>
          </button>
        </li>
      </ul>
      <div :class="PILL_CLASSES">
        <UiButton
          variant="quiet"
          size="sm"
          :class="ATTACH_CLASSES"
          :aria-label="ATTACH_LABEL"
          :title="ATTACH_LABEL"
          :disabled="attachments.saving.value"
          data-testid="conversation-composer-attach"
          @press="attachments.pickFiles()"
        >
          <svg viewBox="0 0 24 24" :class="ICON_SIZE_CLASSES" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="m21.4 11.1-9.2 9.2a6 6 0 0 1-8.5-8.5l9.2-9.2a4 4 0 0 1 5.7 5.7l-9.2 9.2a2 2 0 0 1-2.8-2.8l8.5-8.5" />
          </svg>
        </UiButton>
        <AgentPicker
          v-if="roster.agents.length > 0"
          v-model:open="agentPickerOpen"
          :model-value="selectedMentionRecipient"
          :counts="queuedCounts"
          :count-label="queueCountLabel"
          :disabled="locked"
          busy-first
          placement="top"
          floating="absolute"
          trigger-variant="ghost"
          :trigger-class="selectedMentionRecipient ? 'size-6.5 justify-center !px-0 text-accent' : 'size-6.5 justify-center !px-0 text-muted hover:text-ink'"
          label="Escolher agente destinatário"
          :title="recipientTooltip"
          test-id="conversation-agent-picker"
          @select="onPickRecipient"
        >
          <template #trigger>
            <svg
              class="size-3.5 shrink-0"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <circle cx="12" cy="12" r="4" />
              <path d="M16 8v5a3 3 0 0 0 6 0v-1a10 10 0 1 0-4 8" />
            </svg>
          </template>
        </AgentPicker>
        <AttachmentChips
          :attachments="attachments.attachments.value"
          :saving="attachments.saving.value"
          class="max-w-[45%]"
          @remove="attachments.remove($event)"
        />
        <div class="relative min-w-0 flex-1 overflow-hidden rounded-r-full">
          <div
            aria-hidden="true"
            :class="`pointer-events-none absolute inset-0 whitespace-pre-wrap ${FIELD_TEXT_CLASSES}`"
            :style="{ transform: `translate(${-scrollLeft}px, ${-scrollTop}px)` }"
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
          </div>
          <textarea
            ref="field"
            v-model="draft"
            rows="1"
            role="combobox"
            aria-autocomplete="list"
            :aria-expanded="menuOpen"
            :aria-controls="menuOpen && menuCount > 0 ? listboxId : undefined"
            :aria-activedescendant="activeOptionId"
            :aria-label="FIELD_LABEL"
            :placeholder="placeholder"
            :class="FIELD_CLASSES"
            data-testid="conversation-composer-field"
            @keydown="onFieldKeydown"
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
      <UiButton variant="primary" size="lg" :class="SEND_CLASSES" :aria-label="SEND_LABEL" :title="SEND_LABEL" :disabled="!canSend" @press="submit">
        <svg viewBox="0 0 20 20" class="size-3.5 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M3.5 10.2c4.2-.2 8.4-.1 12.6-.2" />
          <path d="M11.8 5.2c1.6 1.7 3.1 3.3 4.7 4.8-1.6 1.6-3.2 3.2-4.8 4.8" />
        </svg>
      </UiButton>
    </div>
  </div>
</template>
