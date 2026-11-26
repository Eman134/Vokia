<script setup lang="ts">
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import ContextGraphPanel from './ContextGraphPanel.vue'
import MemoryList from './MemoryList.vue'
import LearningSuggestionsPanel from './LearningSuggestionsPanel.vue'
import { useLearningSuggestions } from './useLearningSuggestions'

const learningStore = useLearningSuggestions()

const PAGE_CLASSES = `flex-[999_1_640px] flex min-w-0 flex-col gap-5 overflow-y-auto ${PAGE_INSET_CLASSES}`
const PAGE_TITLE = 'Memória e grafo'
const SECTIONS_CLASSES = 'flex flex-wrap items-start gap-4.5'
const SECTION_CLASSES = 'flex min-w-0 flex-col overflow-hidden rounded-md border border-line bg-surface'
const MEMORY_SECTION_CLASSES = `${SECTION_CLASSES} flex-[1_1_400px]`
const GRAPH_SECTION_CLASSES = `${SECTION_CLASSES} flex-[999_1_560px]`
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
