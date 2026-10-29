<script setup lang="ts">
import { computed } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { CURRENT_PROJECT, PAGE_LABELS, SAMPLE_DATA_LABEL } from '../../shell/railNav'
import ArtifactCard from './ArtifactCard.vue'
import {
  ALL_FILTER,
  ARTIFACT_FILTERS,
  NEW_ARTIFACT_LABEL,
  PAGE_SUBTITLE,
  artifactsOf,
} from './artefatosSample'

const PAGE_CLASSES = 'flex min-w-0 flex-1 flex-col gap-4.5 overflow-y-auto pt-7 pr-8 pb-[150px] pl-2'
const TITLE_CLASSES = 'mt-1 text-[32px] font-bold tracking-[-0.03em] text-ink'
const GRID_CLASSES = 'grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-3.5'

const artifacts = artifactsOf(ALL_FILTER)
const filterOptions = computed(() =>
  ARTIFACT_FILTERS.map((option) => ({
    value: option.value,
    label: `${option.label} ${artifactsOf(option.value).length}`,
  })),
)
const countLabel = `${artifacts.length} artefatos`
</script>

<template>
  <section :class="PAGE_CLASSES">
    <header class="flex flex-wrap items-end gap-4">
      <div>
        <p class="text-caption text-muted">{{ CURRENT_PROJECT }} · {{ SAMPLE_DATA_LABEL }}</p>
        <h1 :class="TITLE_CLASSES">{{ PAGE_LABELS.canvas }}</h1>
        <p class="mt-1 max-w-[70ch] text-body text-muted">{{ PAGE_SUBTITLE }}</p>
      </div>
      <UiButton class="ml-auto" variant="primary" size="lg" disabled aria-disabled="true">
        {{ NEW_ARTIFACT_LABEL }}
      </UiButton>
    </header>

    <div class="flex flex-wrap items-center gap-3">
      <UiSegmented
        :model-value="ALL_FILTER"
        :options="filterOptions"
        label="Tipo de artefato"
        disabled
        wrap
      />
      <span class="ml-auto text-caption text-muted">{{ countLabel }}</span>
    </div>

    <div :class="GRID_CLASSES">
      <ArtifactCard v-for="artifact in artifacts" :key="artifact.title" :artifact="artifact" />
    </div>
  </section>
</template>
