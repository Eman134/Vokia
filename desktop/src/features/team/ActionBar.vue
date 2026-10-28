<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue'
import { AGENTS, useTeam, type AgentProfile } from './useTeam'
import MentionMenu from './MentionMenu.vue'
import UiButton from '../../ui/UiButton.vue'

const RING_BOX = 28
const RING_CENTER = RING_BOX / 2
const RING_RADIUS = 11
const RING_STROKE = 2.5
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS
const SPINNING_ARC = RING_CIRCUMFERENCE / 4
const FULL_RING_DASH = `${RING_CIRCUMFERENCE} 0`
const SPINNING_RING_DASH = `${SPINNING_ARC} ${RING_CIRCUMFERENCE - SPINNING_ARC}`
// Centraliza na janela, mas a borda esquerda nunca entra na coluna fixa do trilho (--rail-w vem de App.vue).
// O invólucro deixa passar o ponteiro: só a pílula captura, para o resto da janela continuar clicável.
const BAR_WRAP_CLASSES =
  'pointer-events-none fixed bottom-6 left-[max(--spacing(6),calc(50%-var(--container-bar)/2),var(--rail-w,0px))] right-[max(--spacing(6),calc(50%-var(--container-bar)/2))] flex justify-center'
// `relative` ancora o menu de @ (MentionMenu é absolute) à pílula, não ao invólucro.
const PILL_CLASSES =
  'pointer-events-auto relative flex h-12 items-center gap-3 rounded-full border border-line bg-surface pl-2.5 pr-1.5 shadow-float transition-[width] duration-180 ease-out'
const COMPACT_WIDTH_CLASS = 'w-80'
const EXPANDED_WIDTH_CLASS = 'w-full'
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
const hovered = ref(false)
const focusWithin = ref(false)

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
const expanded = computed(() => hovered.value || focusWithin.value || menuOpen.value)
const activeAgents = computed(() => AGENTS.filter((agent) => team.states[agent.id] !== 'idle'))
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

function onFocusOut(event: FocusEvent): void {
  if (!(event.currentTarget as HTMLElement).contains(event.relatedTarget as Node | null)) {
    focusWithin.value = false
  }
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
  <div v-if="team.online" :class="BAR_WRAP_CLASSES">
    <form
      :class="[PILL_CLASSES, expanded ? EXPANDED_WIDTH_CLASS : COMPACT_WIDTH_CLASS]"
      @pointerenter="hovered = true"
      @pointerleave="hovered = false"
      @focusin="focusWithin = true"
      @focusout="onFocusOut"
      @submit.prevent="submit"
    >
      <svg
        v-if="activeAgents.length < 2"
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
      <span v-else data-testid="agent-dash" class="flex shrink-0 items-center gap-1">
        <span
          v-for="agent in activeAgents"
          :key="agent.id"
          class="h-1 w-3.5 rounded-full"
          :class="team.states[agent.id] === 'waiting' ? 'bg-warn' : 'bg-ok'"
        />
      </span>
      <span class="w-32 shrink-0 truncate text-label font-semibold text-muted">{{ team.statusText }}</span>
      <template v-if="expanded">
        <span class="h-5.5 w-px shrink-0 bg-line" aria-hidden="true" />
        <UiButton variant="quiet" size="sm" disabled aria-label="Ações da página">☰</UiButton>
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
      </template>
      <input
        ref="inputRef"
        v-model="draft"
        role="combobox"
        aria-autocomplete="list"
        :aria-expanded="menuOpen"
        :aria-controls="menuOpen && mentionMatches.length > 0 ? listboxId : undefined"
        :aria-activedescendant="activeOptionId"
        class="h-8.5 min-w-0 flex-1 truncate rounded-full bg-surface-2 px-3.5 text-label text-ink outline-none placeholder:text-muted focus:outline-2 focus:outline-solid focus:outline-accent disabled:opacity-40"
        :class="{ 'sr-only': !expanded }"
        :placeholder="placeholder"
        :disabled="locked"
        @keydown="onKeydown"
        @blur="dismissMenu"
      />
      <UiButton v-if="expanded" variant="quiet" size="sm" disabled aria-label="Decisões">
        Decisões <span class="font-mono">0</span>
      </UiButton>
      <MentionMenu
        v-if="menuOpen"
        :id="listboxId"
        :agents="mentionMatches"
        :active-index="highlightedIndex"
        @choose="chooseMentionAgent"
      />
    </form>
  </div>
  <div role="status" :class="team.online ? undefined : BAR_WRAP_CLASSES">
    <div v-if="!team.online" :class="[PILL_CLASSES, 'w-full justify-center']">
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
    </div>
  </div>
</template>
