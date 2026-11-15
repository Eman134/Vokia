<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import type { Agent, ConsoleKind } from '../team/events'
import { DOT_CLASSES } from '../team/agentStateStyle'
import { useRoster } from '../team/useRoster'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import UiNotice from '../../ui/UiNotice.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { useFollowEnd } from '../../ui/useFollowEnd'
import ConsoleLines, { KIND_PREFIX } from './ConsoleLines.vue'
import { useConsole } from './useConsole'
import { currentTurn, elapsedLabel, turnOutcome, type TurnOutcome } from './consoleTurn'

type ConsoleFilter = 'all' | 'tools' | 'errors'

const PANEL_TITLE_PREFIX = 'Console de '
const COPY_LABEL = 'Copiar tudo'
const COPIED_LABEL = 'Copiado'
const CLOSE_LABEL = 'Fechar'
const TABS_LABEL = 'Consoles abertos'
const CLOSE_TAB_SYMBOL = '×'
const CLOSE_TAB_PREFIX = 'Fechar console de '
const FILTER_LABEL = 'Filtrar saída'
const JUMP_LABEL = 'Ir para o fim'
const EMPTY_TEXT = 'Nenhuma saída ainda.'
const COPY_ERROR_PREFIX = 'Não foi possível copiar: '
const CLOCK_TICK_MS = 1000
const COPIED_FEEDBACK_MS = 1500

const OUTCOME_DOT: Record<TurnOutcome, string> = {
  running: DOT_CLASSES.thinking,
  done: DOT_CLASSES.idle,
  failed: 'bg-danger',
}
const OUTCOME_BADGE: Record<TurnOutcome, { status: 'trabalhando' | 'livre' | 'erro'; label: string }> = {
  running: { status: 'trabalhando', label: 'ao vivo' },
  done: { status: 'livre', label: 'concluído' },
  failed: { status: 'erro', label: 'falhou' },
}
const FILTERS = [
  { label: 'Tudo', value: 'all' },
  { label: 'Ferramentas', value: 'tools' },
  { label: 'Erros', value: 'errors' },
] as const
const FILTER_KINDS: Record<ConsoleFilter, readonly ConsoleKind[] | null> = {
  all: null,
  tools: ['tool', 'output', 'permission'],
  errors: ['error', 'failed'],
}

const consoleStore = useConsole()
const roster = useRoster()
const filter = ref<ConsoleFilter>('all')
const copied = ref(false)
const copyError = ref<string | null>(null)
const now = ref(Date.now())
const scroller = ref<HTMLElement | null>(null)
const panel = ref<HTMLElement | null>(null)
const { followingEnd, onScroll, followEnd, jumpToEnd } = useFollowEnd(scroller)
let clock: ReturnType<typeof setInterval> | undefined
let copiedTimer: ReturnType<typeof setTimeout> | undefined

const agent = computed<Agent | null>(() => consoleStore.activeAgent)
const title = computed(() => PANEL_TITLE_PREFIX + (agent.value === null ? '' : nameOf(agent.value)))
const entries = computed(() => (agent.value === null ? [] : consoleStore.entriesOf(agent.value)))
const visible = computed(() => {
  const kinds = FILTER_KINDS[filter.value]
  return kinds === null ? entries.value : entries.value.filter((entry) => kinds.includes(entry.kind))
})
const turn = computed(() => currentTurn(entries.value))
const outcome = computed(() => turnOutcome(turn.value))
const elapsed = computed(() => {
  const first = turn.value[0]
  if (first === undefined) return null
  const last = turn.value.at(-1)
  const endMs = outcome.value === 'running' || last === undefined ? now.value : last.time
  return elapsedLabel(first.time, endMs)
})
const activeLoadError = computed(() => (agent.value === null ? null : (consoleStore.loadErrors[agent.value] ?? null)))

function nameOf(id: Agent): string {
  return roster.profileOf(id)?.name ?? id
}

function outcomeOf(id: Agent): TurnOutcome {
  return turnOutcome(currentTurn(consoleStore.entriesOf(id)))
}

function closeActive(): void {
  if (agent.value !== null) consoleStore.close(agent.value)
}

function dismissActiveLoadError(): void {
  if (agent.value !== null) consoleStore.dismissError(agent.value)
}

async function copyAll(): Promise<void> {
  copyError.value = null
  try {
    await navigator.clipboard.writeText(
      visible.value.map((entry) => `${KIND_PREFIX[entry.kind]} ${entry.text}`.trim()).join('\n'),
    )
  } catch (error) {
    copyError.value = `${COPY_ERROR_PREFIX}${String(error)}`
    return
  }
  copied.value = true
  clearTimeout(copiedTimer)
  copiedTimer = setTimeout(() => {
    copied.value = false
  }, COPIED_FEEDBACK_MS)
}

// Esc só vale com o foco dentro do painel: menus, balões e diálogos de fora seguem com o Esc deles.
// Captura para o preventDefault chegar antes do Esc de PanelShell, que só checa defaultPrevented.
function onKeydown(event: KeyboardEvent): void {
  if (event.key !== 'Escape' || agent.value === null) return
  if (!(event.target instanceof Node) || !panel.value?.contains(event.target)) return
  event.preventDefault()
  closeActive()
}

watch(() => visible.value.length, followEnd)
watch(agent, async () => {
  filter.value = 'all'
  void jumpToEnd()
  await nextTick()
  panel.value?.focus()
})

onMounted(() => {
  window.addEventListener('keydown', onKeydown, true)
  clock = setInterval(() => {
    now.value = Date.now()
  }, CLOCK_TICK_MS)
})
onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown, true)
  clearInterval(clock)
  clearTimeout(copiedTimer)
})
</script>

<template>
  <aside
    v-if="agent !== null"
    ref="panel"
    tabindex="-1"
    :aria-label="title"
    class="flex h-screen w-[30rem] max-w-[45vw] shrink-0 flex-col border-l border-line bg-surface focus:outline-none"
  >
    <div role="tablist" :aria-label="TABS_LABEL" class="flex gap-1 overflow-x-auto border-b border-line px-2 pt-2">
      <div
        v-for="id in consoleStore.openAgents"
        :key="id"
        class="flex shrink-0 items-center gap-1 rounded-t-control px-2 py-1"
        :class="id === agent ? 'bg-surface-2' : ''"
      >
        <button
          type="button"
          role="tab"
          :aria-selected="id === agent"
          class="flex items-center gap-1.5 text-caption text-ink focus:outline-2 focus:outline-solid focus:outline-accent"
          @click="consoleStore.activate(id)"
        >
          <span class="size-1.75 rounded-full" :class="OUTCOME_DOT[outcomeOf(id)]" aria-hidden="true" />
          {{ nameOf(id) }}
        </button>
        <button
          type="button"
          :aria-label="CLOSE_TAB_PREFIX + nameOf(id)"
          class="text-caption text-muted focus:outline-2 focus:outline-solid focus:outline-accent"
          @click="consoleStore.close(id)"
        >
          {{ CLOSE_TAB_SYMBOL }}
        </button>
      </div>
    </div>
    <header class="flex items-center gap-2 border-b border-line px-4 py-3">
      <h2 class="text-lead text-ink">{{ title }}</h2>
      <UiBadge :status="OUTCOME_BADGE[outcome].status" :label="OUTCOME_BADGE[outcome].label" />
      <span v-if="elapsed" class="font-mono text-caption text-muted">{{ elapsed }}</span>
      <div class="ml-auto flex items-center gap-1">
        <UiButton variant="quiet" size="sm" @press="copyAll">{{ copied ? COPIED_LABEL : COPY_LABEL }}</UiButton>
        <UiButton variant="quiet" size="sm" @press="closeActive">{{ CLOSE_LABEL }}</UiButton>
      </div>
    </header>
    <div class="px-4 py-2">
      <UiSegmented v-model="filter" :options="FILTERS" :label="FILTER_LABEL" />
    </div>
    <UiNotice v-if="activeLoadError" class="mx-4 mb-2" :text="activeLoadError" @close="dismissActiveLoadError" />
    <UiNotice v-if="copyError" class="mx-4 mb-2" :text="copyError" @close="copyError = null" />
    <div ref="scroller" class="min-h-0 flex-1 overflow-y-auto bg-term-bg px-4 py-3" @scroll="onScroll">
      <ConsoleLines v-if="visible.length > 0" :entries="visible" />
      <p v-else class="font-mono text-small text-term-muted">{{ EMPTY_TEXT }}</p>
    </div>
    <div v-if="!followingEnd" class="px-4 py-2">
      <UiButton variant="primary" size="sm" @press="jumpToEnd">{{ JUMP_LABEL }}</UiButton>
    </div>
  </aside>
</template>
