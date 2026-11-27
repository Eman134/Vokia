<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import { useRoster } from '../team/useRoster'
import ContextGraphPanel from './ContextGraphPanel.vue'
import MemoryList from './MemoryList.vue'
import LearningSuggestionsPanel from './LearningSuggestionsPanel.vue'
import { useLearningSuggestions } from './useLearningSuggestions'

const learningStore = useLearningSuggestions()
const roster = useRoster()

const hasActiveFilters = computed(
  () => learningStore.agentFilter !== null || learningStore.searchQuery.trim() !== '',
)

onMounted(() => {
  if (roster.agents.length === 0) void roster.load()
})

const PAGE_CLASSES = `flex h-full min-h-0 min-w-0 flex-1 flex-col gap-3 overflow-y-auto lg:overflow-hidden ${PAGE_INSET_CLASSES}`
const PAGE_TITLE = 'Memória e grafo'
const SECTIONS_CLASSES = 'flex min-h-0 flex-1 flex-col gap-4 lg:flex-row'
const SECTION_CLASSES = 'flex min-h-0 min-w-0 flex-col overflow-hidden rounded-md border border-line bg-surface'
const MEMORY_SECTION_CLASSES = `${SECTION_CLASSES} max-h-[420px] w-full shrink-0 lg:h-full lg:max-h-none lg:w-[310px] xl:w-[340px]`
const GRAPH_SECTION_CLASSES = `${SECTION_CLASSES} min-h-[420px] flex-1 lg:h-full`
</script>

<template>
  <main :class="PAGE_CLASSES">
    <div class="flex items-center gap-3">
      <UiPageHeader :title="PAGE_TITLE" />
      <span
        v-if="learningStore.pendingCount > 0"
        class="inline-flex items-center rounded-full bg-accent-soft px-2.5 py-0.5 text-tiny font-semibold text-accent"
      >
        {{ learningStore.pendingCount }} pendente{{ learningStore.pendingCount === 1 ? '' : 's' }}
      </span>
    </div>

    <div class="flex flex-wrap items-center gap-2">
      <input
        v-model="learningStore.searchQuery"
        type="search"
        placeholder="Buscar memórias, visão macro, arquivos ou cards…"
        aria-label="Buscar na memória e grafo"
        class="h-8 w-full min-w-[220px] flex-1 rounded-md border border-line bg-surface px-2.5 text-caption text-ink placeholder:text-muted focus:border-ink focus:outline-none sm:max-w-xs"
        data-testid="memory-search"
      />
      <div class="flex flex-wrap items-center gap-1.5" role="group" aria-label="Filtrar por agente">
        <button
          type="button"
          class="cursor-pointer rounded-full border px-2.5 py-1 text-tiny font-medium transition-colors"
          :class="learningStore.agentFilter === null ? 'border-ink bg-ink text-bg' : 'border-line bg-surface text-muted hover:text-ink'"
          @click="learningStore.setAgentFilter(null)"
        >
          Todos
        </button>
        <button
          v-for="agent in roster.agents"
          :key="agent.id"
          type="button"
          class="inline-flex cursor-pointer items-center gap-1.5 rounded-full border px-2.5 py-1 text-tiny font-medium transition-colors"
          :class="learningStore.agentFilter === agent.id ? 'border-ink bg-ink text-bg' : 'border-line bg-surface text-ink hover:bg-hover'"
          @click="learningStore.setAgentFilter(learningStore.agentFilter === agent.id ? null : agent.id)"
        >
          <AgentPortrait :who="agent.id" :size="16" />
          <span>{{ agent.name }}</span>
        </button>
      </div>
      <button
        v-if="hasActiveFilters"
        type="button"
        class="h-8 cursor-pointer rounded-md border border-line bg-surface-2 px-2.5 text-caption text-muted hover:text-ink"
        @click="learningStore.clearFilters()"
      >
        Limpar filtros
      </button>
    </div>

    <LearningSuggestionsPanel title="Memória › Sugestões" />
    <div :class="SECTIONS_CLASSES">
      <section aria-label="Memória" :class="MEMORY_SECTION_CLASSES">
        <MemoryList />
      </section>
      <section aria-label="Grafo do código" :class="GRAPH_SECTION_CLASSES">
        <ContextGraphPanel />
      </section>
    </div>
  </main>
</template>
