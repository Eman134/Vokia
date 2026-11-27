<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import UserAvatar from '../profile/UserAvatar.vue'
import UiButton from '../../ui/UiButton.vue'
import UiMemoryCard from '../../ui/UiMemoryCard.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { useProjects } from '../projects/useProjects'
import type { LearningSuggestion, ProjectMemory } from './learningApi'
import { useLearningSuggestions } from './useLearningSuggestions'

type MemoryFilter =
  | 'projeto'
  | 'macro'
  | 'mapa'
  | 'agente'
  | 'decisoes'
  | 'artefatos'
  | 'devolucoes'
  | 'preferencias'

const FILTER_OPTIONS: readonly { label: string; value: MemoryFilter }[] = [
  { label: 'Todas', value: 'projeto' },
  { label: 'Visão macro', value: 'macro' },
  { label: 'Mapa de arquivos', value: 'mapa' },
  { label: 'Por agente', value: 'agente' },
  { label: 'Decisões', value: 'decisoes' },
  { label: 'Artefatos', value: 'artefatos' },
  { label: 'Devoluções', value: 'devolucoes' },
  { label: 'Preferências', value: 'preferencias' },
]

const MARKER_SIZE = 22
const HEADER_CLASSES = 'flex shrink-0 flex-col items-start gap-2 border-b border-line px-3.5 py-3'
const LIST_CLASSES = 'flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto px-3.5 py-3'
const ROW_CLASSES = 'flex items-start gap-2'
const SUGGESTION_ROW_CLASSES = 'flex items-start gap-2.5 rounded-md bg-accent-soft p-2.5'
const CONTENT_CLASSES = 'flex min-w-0 flex-1 flex-col items-stretch gap-1'
const CHIP_WARN_CLASSES = 'inline-flex items-center rounded-sm bg-warn-soft px-1.5 py-0.5 text-tiny font-medium text-warn-text'
const CHIP_ACCENT_CLASSES = 'inline-flex items-center rounded-sm bg-accent-soft px-1.5 py-0.5 text-tiny font-medium text-accent'
const CHIP_MUTED_CLASSES = 'inline-flex items-center rounded-sm bg-surface-2 px-1.5 py-0.5 text-tiny font-medium text-muted'

const store = useLearningSuggestions()
const projects = useProjects()
const activeFilter = ref<MemoryFilter>('projeto')

interface DisplayMemoryRow {
  key: string
  who: string | null
  fact: string
  author: string
  usage: string
  chip?: { label: string; tone: 'warn' | 'accent' | 'muted' }
}

const pendingMemorySuggestions = computed<LearningSuggestion[]>(() =>
  store.pendingSuggestions.filter((item) => item.kind === 'memory'),
)

function originChip(mem: ProjectMemory): DisplayMemoryRow['chip'] {
  const src = mem.source.toLowerCase()
  if (src.includes('devolu') || src.includes('return')) {
    return { label: 'devolução', tone: 'warn' }
  }
  if (src.includes('artefato') || src.includes('artifact') || src.includes('adr')) {
    return { label: 'artefato', tone: 'accent' }
  }
  if (mem.originType === 'manual' || mem.pinned) {
    return { label: mem.pinned ? 'fixada' : 'manual', tone: 'muted' }
  }
  return undefined
}

const persistedRows = computed<DisplayMemoryRow[]>(() =>
  store.memories.map((mem) => ({
    key: mem.id,
    who: mem.author ?? null,
    fact: mem.fact,
    author: mem.author ?? (mem.originType === 'manual' ? 'sua preferência' : 'Time'),
    usage:
      mem.originType === 'manual'
        ? 'sua preferência · vale para todos os agentes'
        : `${mem.source} · ${mem.originType}`,
    chip: originChip(mem),
  })),
)

const filteredRows = computed<DisplayMemoryRow[]>(() => {
  let rows = persistedRows.value
  if (store.agentFilter) {
    rows = rows.filter((r) => r.who === store.agentFilter)
  }
  const query = store.searchQuery.trim().toLowerCase()
  if (query) {
    rows = rows.filter(
      (r) =>
        r.fact.toLowerCase().includes(query) ||
        r.author.toLowerCase().includes(query) ||
        r.usage.toLowerCase().includes(query),
    )
  }
  switch (activeFilter.value) {
    case 'macro':
      return rows.filter(
        (r) =>
          r.usage.toLowerCase().includes('macro') ||
          r.usage.toLowerCase().includes('arquitetura') ||
          r.chip?.label === 'fixada',
      )
    case 'mapa':
      return rows.filter(
        (r) =>
          r.usage.toLowerCase().includes('mapa') ||
          r.fact.includes('/') ||
          r.fact.includes('.rs') ||
          r.fact.includes('.ts') ||
          r.fact.includes('.vue'),
      )
    case 'agente':
      return rows.filter((r) => r.who !== null)
    case 'decisoes':
      return rows.filter((r) => r.usage.toLowerCase().includes('decis') || r.usage.toLowerCase().includes('adr'))
    case 'artefatos':
      return rows.filter((r) => r.chip?.tone === 'accent' || r.usage.toLowerCase().includes('artefato') || r.usage.toLowerCase().includes('diagrama'))
    case 'devolucoes':
      return rows.filter((r) => r.chip?.tone === 'warn' || r.usage.toLowerCase().includes('devolu'))
    case 'preferencias':
      return rows.filter((r) => r.who === null || r.usage.includes('sua preferência'))
    case 'projeto':
    default:
      return rows
  }
})

function chipClass(tone: 'warn' | 'accent' | 'muted'): string {
  if (tone === 'warn') return CHIP_WARN_CLASSES
  if (tone === 'accent') return CHIP_ACCENT_CLASSES
  return CHIP_MUTED_CLASSES
}

function isBusy(id: string): boolean {
  return store.busyIds.has(id)
}

onMounted(() => {
  void store.bindEvents()
  void store.load(projects.selected?.id)
})
</script>

<template>
  <div :class="HEADER_CLASSES">
    <div class="flex w-full flex-wrap items-center justify-between gap-2">
      <h2 class="text-lead text-ink">Memória</h2>
      <span
        v-if="pendingMemorySuggestions.length > 0"
        class="inline-flex items-center rounded-full bg-accent-soft px-2 py-0.5 text-tiny font-semibold text-accent"
      >
        {{ pendingMemorySuggestions.length }} para confirmar
      </span>
    </div>
    <UiSegmented
      :options="FILTER_OPTIONS"
      :model-value="activeFilter"
      label="Filtrar memórias por origem"
      @update:model-value="activeFilter = $event as MemoryFilter"
    />
  </div>

  <ol :class="LIST_CLASSES">
    <li
      v-for="sug in pendingMemorySuggestions"
      :key="sug.id"
      :class="SUGGESTION_ROW_CLASSES"
      :data-testid="`inline-memory-suggestion-${sug.id}`"
    >
      <UserAvatar v-if="!sug.sourceAgent" :size="MARKER_SIZE" />
      <AgentPortrait v-else :who="sug.sourceAgent" :size="MARKER_SIZE" />
      <div :class="CONTENT_CLASSES">
        <div class="flex flex-wrap items-center gap-2">
          <span class="text-caption font-medium text-accent">
            {{ sug.sourceAgent ?? 'Time' }} sugeriu · espera você confirmar
          </span>
          <span v-if="sug.sourceCardId" :class="CHIP_ACCENT_CLASSES">{{ sug.sourceCardId }}</span>
        </div>
        <p class="text-body text-ink">{{ sug.content }}</p>
        <p v-if="sug.rationale" class="text-small text-muted">{{ sug.rationale }}</p>
        <div class="flex items-center justify-end gap-2 pt-1">
          <UiButton
            variant="quiet"
            size="sm"
            :disabled="isBusy(sug.id)"
            @press="store.dismiss(sug.id)"
          >
            Descartar
          </UiButton>
          <UiButton
            variant="primary"
            size="sm"
            :disabled="isBusy(sug.id)"
            @press="store.apply(sug.id)"
          >
            {{ isBusy(sug.id) ? 'Guardando...' : 'Guardar' }}
          </UiButton>
        </div>
      </div>
    </li>

    <li v-for="fact in filteredRows" :key="fact.key" :class="ROW_CLASSES">
      <UserAvatar v-if="fact.who === null" :size="MARKER_SIZE" />
      <AgentPortrait v-else :who="fact.who" :size="MARKER_SIZE" />
      <div :class="CONTENT_CLASSES">
        <div v-if="fact.chip" class="flex items-center gap-1.5">
          <span :class="chipClass(fact.chip.tone)">{{ fact.chip.label }}</span>
        </div>
        <UiMemoryCard class="w-full" :fact="fact.fact" :author="fact.author" :usage="fact.usage" />
      </div>
    </li>
  </ol>
</template>
