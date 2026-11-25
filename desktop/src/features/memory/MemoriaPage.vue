<script setup lang="ts">
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import MemoryList from './MemoryList.vue'
import { GRAPH_DESCRIPTION, GRAPH_EMPTY_TITLE } from './memoriaSample'

const PAGE_CLASSES = `flex-[999_1_640px] flex min-w-0 flex-col gap-5 overflow-y-auto ${PAGE_INSET_CLASSES}`
const PAGE_TITLE = 'Memória e grafo'
const SECTIONS_CLASSES = 'flex flex-wrap items-start gap-4.5'
const SECTION_CLASSES = 'flex min-w-0 flex-col overflow-hidden rounded-md border border-line bg-surface'
const MEMORY_SECTION_CLASSES = `${SECTION_CLASSES} flex-[1_1_400px]`
const GRAPH_SECTION_CLASSES = `${SECTION_CLASSES} flex-[999_1_560px]`
const SECTION_HEADER_CLASSES = 'flex flex-wrap items-baseline gap-2 px-4.5 py-4'
</script>

<template>
  <main :class="PAGE_CLASSES">
    <UiPageHeader :title="PAGE_TITLE" />
    <div :class="SECTIONS_CLASSES">
      <section aria-label="Memória" :class="MEMORY_SECTION_CLASSES">
        <MemoryList />
      </section>
      <section aria-label="Grafo do código" :class="GRAPH_SECTION_CLASSES">
        <div :class="SECTION_HEADER_CLASSES">
          <h2 class="text-lead text-ink">Grafo</h2>
          <p class="text-caption text-muted">{{ GRAPH_DESCRIPTION }}</p>
        </div>
        <UiEmpty class="px-4.5 pb-6" :title="GRAPH_EMPTY_TITLE" />
      </section>
    </div>
  </main>
</template>
