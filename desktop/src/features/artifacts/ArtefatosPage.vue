<script setup lang="ts">
import { computed } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { PAGE_LABELS } from '../../shell/railNav'
import { useProjects } from '../projects/useProjects'
import ArtifactCard from './ArtifactCard.vue'
import {
  ALL_FILTER,
  ARTIFACT_FILTERS,
  NEW_ARTIFACT_LABEL,
  PAGE_SUBTITLE,
  artifactsOf,
} from './artefatosSample'

const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col gap-4.5 overflow-y-auto ${PAGE_INSET_CLASSES}`
const GRID_CLASSES = 'grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-3.5'

const artifacts = artifactsOf(ALL_FILTER)
const filterOptions = computed(() =>
  ARTIFACT_FILTERS.map((option) => ({
    value: option.value,
    label: `${option.label} ${artifactsOf(option.value).length}`,
  })),
)
const countLabel = `${artifacts.length} artefatos`
const projectsStore = useProjects()
const projectName = computed(() => projectsStore.selected?.name ?? '')
</script>

<template>
  <section :class="PAGE_CLASSES">
    <UiPageHeader :title="PAGE_LABELS.canvas" :project-name="projectName" :subtitle="PAGE_SUBTITLE">
      <template #actions>
        <UiButton variant="primary" size="lg" disabled aria-disabled="true">
          {{ NEW_ARTIFACT_LABEL }}
        </UiButton>
      </template>
    </UiPageHeader>

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
