<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import UiCreateButton from '../../ui/UiCreateButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import { useNavigation } from '../../shell/useNavigation'
import { useFeed } from '../team/useFeed'
import { useConsole } from '../console/useConsole'
import ProjectCard from './ProjectCard.vue'
import ProjectEditForm from './ProjectEditForm.vue'
import { NEW_PROJECT_LABEL, PROJECTS_TITLE } from './projetosSample'
import {
  isProjectError,
  type PermissionLevel,
  type Project,
  type ProjectEdit,
} from './projectsApi'
import { useProjects } from './useProjects'

const NO_PROJECTS_TEXT = 'Nenhum projeto ainda. Crie o primeiro para começar.'
const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto ${PAGE_INSET_CLASSES}`
const GRID_CLASSES = 'grid grid-cols-[repeat(auto-fill,minmax(min(440px,100%),1fr))] gap-4.5'

const navigation = useNavigation()
const projectsStore = useProjects()
const feed = useFeed()
const consoleStore = useConsole()
const failure = ref<string | null>(null)
const editingId = ref<number | null>(null)
const editing = computed(() => projectsStore.projects.find((project) => project.id === editingId.value) ?? null)

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

async function save(project: Project, input: ProjectEdit, level: PermissionLevel) {
  try {
    await projectsStore.update(project.id, input)
    if (level !== project.permissionLevel) await projectsStore.setPermission(project.id, level)
    editingId.value = null
  } catch (cause) {
    failure.value = describe(cause)
  }
}

async function remove(project: Project) {
  try {
    await projectsStore.remove(project.id)
    editingId.value = null
    consoleStore.forgetProject(project.id)
    if (project.selected) await feed.restoreHistory()
  } catch (cause) {
    failure.value = describe(cause)
  }
}

async function togglePin(project: Project) {
  try {
    await projectsStore.pin(project.pinned ? null : project.id)
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
    <UiPageHeader :title="PROJECTS_TITLE">
      <template #actions>
        <UiCreateButton data-testid="new-project" @press="navigation.goTo('novo-projeto')">
          {{ NEW_PROJECT_LABEL }}
        </UiCreateButton>
      </template>
    </UiPageHeader>

    <UiNotice v-if="failure" :text="failure" @close="failure = null" />

    <UiEmpty v-if="projectsStore.loaded && projectsStore.projects.length === 0" :title="NO_PROJECTS_TEXT" />

    <div :class="GRID_CLASSES">
      <ProjectCard
        v-for="project in projectsStore.projects"
        :key="project.id"
        :project="project"
        @open="open(project)"
        @edit="editingId = project.id"
        @pin="togglePin(project)"
      />
    </div>
    <ProjectEditForm
      v-if="editing"
      :project="editing"
      @save="(input, level) => save(editing!, input, level)"
      @cancel="editingId = null"
      @remove="remove(editing!)"
    />
  </section>
</template>
