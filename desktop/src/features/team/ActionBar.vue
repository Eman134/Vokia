<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue'
import { AGENTS, useTeam, type AgentProfile } from './useTeam'
import MentionMenu from './MentionMenu.vue'

const RING_BOX = 28
const RING_CENTER = RING_BOX / 2
const RING_RADIUS = 11
const RING_STROKE = 2.5
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS
const SPINNING_ARC = RING_CIRCUMFERENCE / 4
const FULL_RING_DASH = `${RING_CIRCUMFERENCE} 0`
const SPINNING_RING_DASH = `${SPINNING_ARC} ${RING_CIRCUMFERENCE - SPINNING_ARC}`
const PILL_CLASSES =
  'fixed bottom-6 inset-x-6 mx-auto flex h-12 max-w-190 items-center gap-3 rounded-full border border-line bg-surface pl-2.5 pr-1.5 shadow-float'
const CHIP_CLASSES =
  'flex h-7 shrink-0 items-center gap-1 rounded-full bg-accent-soft py-0.5 pl-3 pr-1 text-label font-semibold text-accent'
const CHIP_REMOVE_CLASSES =
  'grid size-5 cursor-pointer place-items-center rounded-full hover:bg-surface focus:outline-2 focus:outline-solid focus:outline-accent disabled:opacity-40'
const DEFAULT_PLACEHOLDER = 'Peça algo ao time'
const MENTION_QUERY_PATTERN = /^@(\S*)$/

const team = useTeam()
const draft = ref('')
const inputRef = ref<HTMLInputElement | null>(null)
const listboxId = useId()
const menuDismissed = ref(false)
const highlightedIndex = ref(0)

const locked = computed(() => team.busy || team.checking)
const recipientProfile = computed(() => AGENTS.find((agent) => agent.id === team.recipient))
const placeholder = computed(() =>
  recipientProfile.value ? `Mensagem para ${recipientProfile.value.name}` : DEFAULT_PLACEHOLDER,
)
const mentionQuery = computed<string | null>(() => MENTION_QUERY_PATTERN.exec(draft.value)?.[1] ?? null)
const mentionMatches = computed<AgentProfile[]>(() => {
  const query = mentionQuery.value
  if (query === null) {
    return []
  }
  const prefix = query.toLowerCase()
  return AGENTS.filter((agent) => agent.name.toLowerCase().startsWith(prefix))
})
const highlightedAgent = computed(() => mentionMatches.value.at(highlightedIndex.value))
const menuOpen = computed(() => mentionQuery.value !== null && !menuDismissed.value && !locked.value)
const activeOptionId = computed(() =>
  menuOpen.value && highlightedAgent.value ? `${listboxId}-${highlightedAgent.value.id}` : undefined,
)

watch(mentionQuery, () => {
  menuDismissed.value = false
  highlightedIndex.value = 0
})

watch(
  () => team.recipient,
  () => inputRef.value?.focus(),
)

async function submit(): Promise<void> {
  const text = draft.value.trim()
  if (text === '' || locked.value) {
    return
  }
  const pending = draft.value
  draft.value = ''
  if (!(await team.send(text))) {
    draft.value = pending
  }
}

function chooseMentionAgent(agent: AgentProfile): void {
  team.chooseRecipient(agent.id)
  draft.value = ''
}

function dismissMenu(): void {
  menuDismissed.value = true
}

function moveHighlight(step: number): void {
  const count = mentionMatches.value.length
  if (count === 0) {
    return
  }
  highlightedIndex.value = (highlightedIndex.value + step + count) % count
}

function handleMentionKey(event: KeyboardEvent): void {
  switch (event.key) {
    case 'ArrowDown':
      event.preventDefault()
      moveHighlight(1)
      break
    case 'ArrowUp':
      event.preventDefault()
      moveHighlight(-1)
      break
    case 'Enter':
    case 'Tab': {
      const agent = highlightedAgent.value
      if (agent === undefined && event.key === 'Tab') {
        break
      }
      // Enter com a lista aberta nunca envia, mesmo sem resultado.
      event.preventDefault()
      if (agent !== undefined) {
        chooseMentionAgent(agent)
      }
      break
    }
    case 'Escape':
      event.preventDefault()
      dismissMenu()
      break
  }
}

function onKeydown(event: KeyboardEvent): void {
  if (menuOpen.value) {
    handleMentionKey(event)
    return
  }
  if (event.key === 'Backspace' && draft.value === '' && team.recipient !== null) {
    team.clearRecipient()
  }
}
</script>

<template>
  <form v-if="team.online" :class="PILL_CLASSES" @submit.prevent="submit">
    <svg
      class="size-6.5 shrink-0"
      :class="{ 'animate-spin': team.thinkingAgent }"
      :viewBox="`0 0 ${RING_BOX} ${RING_BOX}`"
      fill="none"
      :stroke-width="RING_STROKE"
      aria-hidden="true"
    >
      <circle :cx="RING_CENTER" :cy="RING_CENTER" :r="RING_RADIUS" class="stroke-line" />
      <circle
        :cx="RING_CENTER"
        :cy="RING_CENTER"
        :r="RING_RADIUS"
        class="stroke-ok"
        stroke-linecap="round"
        :stroke-dasharray="team.thinkingAgent ? SPINNING_RING_DASH : FULL_RING_DASH"
      />
    </svg>
    <span class="w-32 shrink-0 truncate text-label font-semibold text-muted">{{ team.statusText }}</span>
    <span class="h-5.5 w-px shrink-0 bg-line" aria-hidden="true" />
    <span v-if="recipientProfile" :class="CHIP_CLASSES">
      {{ recipientProfile.name }}
      <button
        type="button"
        :class="CHIP_REMOVE_CLASSES"
        :aria-label="`Remover destinatário ${recipientProfile.name}`"
        :disabled="locked"
        @click="team.clearRecipient()"
      >
        <span aria-hidden="true">×</span>
      </button>
    </span>
    <input
      ref="inputRef"
      v-model="draft"
      role="combobox"
      aria-autocomplete="list"
      :aria-expanded="menuOpen"
      :aria-controls="menuOpen && mentionMatches.length > 0 ? listboxId : undefined"
      :aria-activedescendant="activeOptionId"
      class="h-8.5 min-w-0 flex-1 rounded-full bg-surface-2 px-3.5 text-label text-ink outline-none placeholder:text-muted focus:outline-2 focus:outline-solid focus:outline-accent disabled:opacity-40"
      :placeholder="placeholder"
      :disabled="locked"
      @keydown="onKeydown"
      @blur="dismissMenu"
    />
    <MentionMenu
      v-if="menuOpen"
      :id="listboxId"
      :agents="mentionMatches"
      :active-index="highlightedIndex"
      @choose="chooseMentionAgent"
    />
  </form>
  <div role="status" :class="team.online ? undefined : [PILL_CLASSES, 'justify-center']">
    <template v-if="!team.online">
      <svg
        class="size-4 shrink-0 text-danger"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.4"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M12 19.5h.01" />
        <path d="M8.5 15.5a5 5 0 0 1 7 0" />
        <path d="M5 12a10 10 0 0 1 14 0" />
        <path d="M2 8.8a15 15 0 0 1 20 0" />
        <path d="M3 3l18 18" />
      </svg>
      <span class="text-label font-semibold text-danger">Sem conexão</span>
    </template>
  </div>
</template>
