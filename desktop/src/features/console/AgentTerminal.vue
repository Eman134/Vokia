<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { agentName } from '../team/agents'
import { STATE_LABELS } from '../team/agentStateStyle'
import { useFeed } from '../team/useFeed'
import { useRequests } from '../team/useRequests'
import { useRequestNotices } from '../balloon/useRequestNotices'
import { useProjects } from '../projects/useProjects'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { useFollowEnd } from '../../ui/useFollowEnd'
import { currentTurn } from './consoleTurn'
import { buildRows, type DiffTone, type NoteTone } from './terminalRows'
import { useConsole } from './useConsole'

const NO_AGENT_TITLE = 'Escolha um agente na lista para ver o terminal dele.'
const ANSWER_LABEL = 'Responder em Conversa'
const TERMINAL_LABEL_PREFIX = 'Terminal de '
const TITLE_SEPARATOR = ' · '
const TITLE_NAME = 'claude'
const EMPTY_TEXT = 'Nenhuma saída ainda.'
const JUMP_LABEL = 'Ir para o fim'
const SHORTCUTS = 'enter enviar · esc sair'
const PROMPT_MARK = '>'
const BRANCH = '⎿'
const USER = 'user'
const MS_PER_SECOND = 1000
const CLOCK_TICK_MS = 1000

const TONE_CLASSES: Record<DiffTone, string> = {
  plain: '',
  add: 'bg-term-ok/15 text-term-ok',
  remove: 'bg-term-danger/15 text-term-danger',
}
const NOTE_CLASSES: Record<NoteTone, string> = {
  ok: 'text-term-ok',
  danger: 'text-term-danger',
  warn: 'text-term-warn',
}

const emit = defineEmits<{ answer: []; exit: [] }>()
const feed = useFeed()
const consoleStore = useConsole()
const projects = useProjects()
const requests = useRequests()
const notices = useRequestNotices()
const draft = ref('')
const now = ref(Date.now())
const scroller = ref<HTMLElement | null>(null)
const { followingEnd, onScroll, followEnd, jumpToEnd } = useFollowEnd(scroller)
let clock: ReturnType<typeof setInterval> | undefined

const agent = computed(() => (feed.screen.kind === 'agent' ? feed.screen.agent : null))
const entries = computed(() => (agent.value === null ? [] : consoleStore.entriesOf(agent.value)))
const prompts = computed(() =>
  feed.feed.flatMap((entry) =>
    entry.type === 'message' &&
    entry.from === USER &&
    entry.to === agent.value &&
    (entry.projectId === null || entry.projectId === projects.selected?.id)
      ? [{ time: entry.time, text: entry.text }]
      : [],
  ),
)
const rows = computed(() => buildRows(entries.value, prompts.value))
const loadError = computed(() => (agent.value === null ? undefined : consoleStore.loadErrors[agent.value]))
const waiting = computed(() => agent.value !== null && feed.permissions.some((permission) => permission.agent === agent.value))
const working = computed(() => agent.value !== null && feed.activeAgents.some((profile) => profile.id === agent.value))
const progress = computed(() => {
  if (agent.value === null || !working.value) return null
  const startedAt = currentTurn(entries.value)[0]?.time
  const seconds = startedAt === undefined ? null : Math.max(0, Math.floor((now.value - startedAt) / MS_PER_SECOND))
  const label = STATE_LABELS[feed.stateOf(agent.value)]
  return seconds === null ? `${label}…` : `${label}… (${seconds}s)`
})

async function send(): Promise<void> {
  const text = draft.value.trim()
  if (text === '' || agent.value === null) return
  const pending = draft.value
  draft.value = ''
  const accepted = await requests.send(text, agent.value, {
    onEnqueued: notices.announceSent,
    onQueued: notices.markBackendQueued,
  })
  if (!accepted) draft.value = pending
}

watch(
  [agent, () => projects.selected?.id],
  () => {
    if (agent.value !== null) void consoleStore.load(agent.value)
    void nextTick().then(jumpToEnd)
  },
  { immediate: true },
)
watch(() => rows.value.length, followEnd)
onMounted(() => {
  void followEnd()
  clock = setInterval(() => {
    now.value = Date.now()
  }, CLOCK_TICK_MS)
})
onUnmounted(() => clearInterval(clock))
</script>

<template>
  <section
    v-if="agent !== null"
    :aria-label="TERMINAL_LABEL_PREFIX + agentName(agent)"
    class="m-3 flex min-h-0 flex-1 flex-col overflow-hidden rounded-card border border-term-line bg-term-bg font-mono text-label leading-relaxed text-term-ink"
  >
    <div class="truncate border-b border-term-line px-4 py-2 text-small text-term-muted">
      {{ TITLE_NAME }}{{ TITLE_SEPARATOR }}{{ projects.selected?.path }}
    </div>
    <div v-if="waiting" class="flex items-center justify-between gap-3 border-b border-term-line px-4 py-2">
      <span class="text-term-warn">{{ agentName(agent) }} espera sua permissão.</span>
      <UiButton variant="primary" size="sm" @press="emit('answer')">{{ ANSWER_LABEL }}</UiButton>
    </div>
    <UiNotice v-if="loadError" class="m-3" :text="loadError" @close="consoleStore.dismissError(agent)" />
    <div ref="scroller" class="min-h-0 flex-1 overflow-y-auto px-4 py-3" @scroll="onScroll">
      <p v-if="rows.length === 0 && progress === null" class="text-term-muted">{{ EMPTY_TEXT }}</p>
      <ol class="flex flex-col gap-2">
        <li v-for="row in rows" :key="row.key" :data-kind="row.type">
          <div v-if="row.type === 'prompt'" class="flex gap-2">
            <span class="select-none font-bold" aria-hidden="true">{{ PROMPT_MARK }}</span>
            <span class="min-w-0 font-bold wrap-break-word whitespace-pre-wrap">{{ row.text }}</span>
          </div>
          <div v-else-if="row.type === 'tool'" class="flex gap-2">
            <span class="mt-2 size-1.5 shrink-0 rounded-full bg-term-ok" aria-hidden="true" />
            <span class="min-w-0 wrap-break-word"><b>{{ row.name }}</b><template v-if="row.argument">({{ row.argument }})</template></span>
          </div>
          <div v-else-if="row.type === 'agent'" class="flex gap-2">
            <span class="mt-2 size-1.5 shrink-0 rounded-full bg-term-ink" aria-hidden="true" />
            <span class="min-w-0 wrap-break-word whitespace-pre-wrap">{{ row.text }}</span>
          </div>
          <div v-else-if="row.type === 'result'" class="flex gap-2 pl-4" :class="row.failed ? 'text-term-danger' : 'text-term-muted'">
            <span class="select-none" aria-hidden="true">{{ BRANCH }}</span>
            <div class="min-w-0 flex-1">
              <div v-for="(line, index) in row.lines" :key="index" class="wrap-break-word whitespace-pre-wrap" :class="TONE_CLASSES[line.tone]">{{ line.text }}</div>
            </div>
          </div>
          <div v-else-if="row.type === 'diff'" class="pl-9">
            <div v-for="(line, index) in row.lines" :key="index" class="wrap-break-word whitespace-pre-wrap" :class="TONE_CLASSES.add">{{ line }}</div>
          </div>
          <div v-else class="pl-4" :class="NOTE_CLASSES[row.tone]">{{ row.text }}</div>
        </li>
      </ol>
      <p v-if="progress !== null" class="mt-2 flex items-center gap-2 text-term-muted" role="status">
        <span class="size-1.5 shrink-0 animate-pulse rounded-full bg-term-muted motion-reduce:animate-none" aria-hidden="true" />
        {{ progress }}
      </p>
    </div>
    <div v-if="!followingEnd" class="px-4 py-2">
      <UiButton variant="primary" size="sm" @press="jumpToEnd">{{ JUMP_LABEL }}</UiButton>
    </div>
    <div class="flex flex-col gap-1 border-t border-term-line px-4 py-3">
      <form class="flex items-center gap-2 rounded-control border border-term-line px-3 py-1.5" @submit.prevent="send">
        <span class="select-none font-bold" aria-hidden="true">{{ PROMPT_MARK }}</span>
        <input
          v-model="draft"
          type="text"
          class="min-w-0 flex-1 bg-transparent text-term-ink placeholder:text-term-muted focus:outline-none"
          :aria-label="`fale com ${agentName(agent)}`"
          :placeholder="`fale com ${agentName(agent)}`"
          @keydown.esc="emit('exit')"
        />
      </form>
      <span class="text-small text-term-muted">{{ SHORTCUTS }}</span>
    </div>
  </section>
  <UiEmpty v-else class="m-auto" :title="NO_AGENT_TITLE" />
</template>
