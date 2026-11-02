<script setup lang="ts">
import { onMounted, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { useNavigation } from '../../shell/useNavigation'
import { useTeam } from '../team/useTeam'
import ProjectCard from './ProjectCard.vue'
import ProjectEditForm from './ProjectEditForm.vue'
import { NEW_PROJECT_LABEL, PROJECTS_SUBTITLE, PROJECTS_TITLE } from './projetosSample'
import { isProjectError, type Project, type ProjectEdit } from './projectsApi'
import { useProjects } from './useProjects'

const PROJECTS_CONTEXT = 'Workspace'
const NO_PROJECTS_TEXT = 'Nenhum projeto ainda. Crie o primeiro para começar.'
const PAGE_CLASSES = 'flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto pt-7 pr-8 pb-[150px] pl-2'
const HEADER_CLASSES = 'flex flex-wrap items-end gap-4'
const TITLE_CLASSES = 'mt-1 text-[32px] font-bold tracking-[-0.03em] text-ink'
const SUBTITLE_CLASSES = 'mt-1 max-w-[64ch] text-body text-muted'
const GRID_CLASSES = 'grid grid-cols-[repeat(auto-fill,minmax(440px,1fr))] gap-4.5'

const navigation = useNavigation()
const projectsStore = useProjects()
const team = useTeam()
const failure = ref<string | null>(null)
const editingId = ref<number | null>(null)

function describe(cause: unknown): string {
  if (isProjectError(cause)) return cause.message
  return cause instanceof Error ? cause.message : String(cause)
}

async function open(project: Project) {
  try {
    await navigation.enterProject(project)
  } catch (cause) {
    failure.value = describe(cause)
  }
}

async function save(project: Project, input: ProjectEdit) {
  try {
    await projectsStore.update(project.id, input)
    editingId.value = null
  } catch (cause) {
    failure.value = describe(cause)
  }
}

async function remove(project: Project) {
  try {
    await projectsStore.remove(project.id)
    if (project.selected) await team.restoreHistory()
  } catch (cause) {
    failure.value = describe(cause)
  }
}

onMounted(async () => {
  try {
    await projectsStore.load()
  } catch (cause) {
    failure.value = describe(cause)
  }
})
</script>

<template>
  <section :class="PAGE_CLASSES">
    <header :class="HEADER_CLASSES">
      <div>
        <p class="text-caption text-muted" data-testid="context-label">
          {{ PROJECTS_CONTEXT }}
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

    <UiEmpty v-if="projectsStore.loaded && projectsStore.projects.length === 0" :title="NO_PROJECTS_TEXT" />

    <div :class="GRID_CLASSES">
      <template v-for="project in projectsStore.projects" :key="project.id">
        <ProjectEditForm
          v-if="editingId === project.id"
          :project="project"
          @save="(input) => save(project, input)"
          @cancel="editingId = null"
        />
        <ProjectCard
          v-else
          :project="project"
          @open="open(project)"
          @edit="editingId = project.id"
          @remove="remove(project)"
        />
      </template>
    </div>
  </section>
</template>
