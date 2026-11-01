<script setup lang="ts">
import { onMounted, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { SAMPLE_DATA_LABEL } from '../../shell/railNav'
import { useNavigation } from '../../shell/useNavigation'
import ProjectCard from './ProjectCard.vue'
import SampleProjectCard from './SampleProjectCard.vue'
import {
  NEW_PROJECT_LABEL,
  PROJECTS_SUBTITLE,
  PROJECTS_TITLE,
  SAMPLE_PROJECTS,
} from './projetosSample'
import { listProjects, type Project } from './projectsApi'
import { useProjects } from './useProjects'

const PROJECTS_CONTEXT = 'Workspace'
const PAGE_CLASSES = 'flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto pt-7 pr-8 pb-[150px] pl-2'
const HEADER_CLASSES = 'flex flex-wrap items-end gap-4'
const TITLE_CLASSES = 'mt-1 text-[32px] font-bold tracking-[-0.03em] text-ink'
const SUBTITLE_CLASSES = 'mt-1 max-w-[64ch] text-body text-muted'
const GRID_CLASSES = 'grid grid-cols-[repeat(auto-fill,minmax(440px,1fr))] gap-4.5'

const navigation = useNavigation()
const projectsStore = useProjects()
const projects = ref<Project[]>([])
const failure = ref<string | null>(null)

function describe(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause)
}

async function open(project: Project) {
  try {
    await projectsStore.select(project.id)
    navigation.goTo('conversas')
  } catch (cause) {
    failure.value = describe(cause)
  }
}

onMounted(async () => {
  try {
    projects.value = await listProjects()
  } catch (cause) {
    failure.value = describe(cause)
  }
})
</script>

<template>
  <section :class="PAGE_CLASSES">
    <header :class="HEADER_CLASSES">
      <div>
        <p class="text-caption text-muted" data-testid="sample-label">
          {{ PROJECTS_CONTEXT }} · {{ SAMPLE_DATA_LABEL }}
        </p>
        <h1 :class="TITLE_CLASSES">{{ PROJECTS_TITLE }}</h1>
        <p :class="SUBTITLE_CLASSES">{{ PROJECTS_SUBTITLE }}</p>
      </div>
      <UiButton
        class="ml-auto"
        variant="primary"
        size="lg"
        data-testid="new-project"
        @press="navigation.goTo('novo-projeto')"
      >
        {{ NEW_PROJECT_LABEL }}
      </UiButton>
    </header>

    <UiNotice v-if="failure" :text="failure" @close="failure = null" />

    <div :class="GRID_CLASSES">
      <ProjectCard v-for="project in projects" :key="project.id" :project="project" @open="open(project)" />
      <SampleProjectCard v-for="sample in SAMPLE_PROJECTS" :key="sample.name" :project="sample" />
    </div>
  </section>
</template>
