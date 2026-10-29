<script setup lang="ts">
import { CURRENT_PROJECT, SAMPLE_DATA_LABEL } from '../../shell/railNav'
import UiEmpty from '../../ui/UiEmpty.vue'
import MemoryList from './MemoryList.vue'
import { GRAPH_DESCRIPTION, GRAPH_EMPTY_TITLE } from './memoriaSample'

const PAGE_CLASSES = 'flex-[999_1_640px] flex min-w-0 flex-col gap-5 overflow-y-auto pb-[150px] pl-2 pr-8 pt-7'
const TITLE_CLASSES = 'mt-1 text-[32px] font-bold tracking-[-0.03em] text-ink'
const SECTIONS_CLASSES = 'flex flex-wrap items-start gap-4.5'
const SECTION_CLASSES = 'flex min-w-0 flex-col overflow-hidden rounded-dialog border border-line bg-surface'
const MEMORY_SECTION_CLASSES = `${SECTION_CLASSES} flex-[1_1_400px]`
const GRAPH_SECTION_CLASSES = `${SECTION_CLASSES} flex-[999_1_560px]`
const SECTION_HEADER_CLASSES = 'flex flex-wrap items-baseline gap-2 px-4.5 py-4'
</script>

<template>
  <main :class="PAGE_CLASSES">
    <header>
      <span class="text-caption text-muted">{{ CURRENT_PROJECT }} · {{ SAMPLE_DATA_LABEL }}</span>
      <h1 :class="TITLE_CLASSES">Memória e grafo</h1>
      <p class="mt-1 max-w-[64ch] text-body text-muted">
        O que os agentes aprenderam sobre o projeto e o mapa que liga código e artefatos (diagramas, documentos e
        decisões) para ler menos e trabalhar sem se atropelar.
      </p>
    </header>
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
