import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import {
  deleteProject,
  listProjects,
  selectProject,
  updateProject,
  type Project,
  type ProjectEdit,
} from './projectsApi'

export const useProjects = defineStore('projects', () => {
  const projects = ref<Project[]>([])
  const loaded = ref(false)
  const selected = computed(() => projects.value.find((project) => project.selected) ?? null)

  async function load(): Promise<void> {
    projects.value = await listProjects()
    loaded.value = true
  }

  async function select(id: number): Promise<void> {
    await selectProject(id)
    await load()
  }

  async function update(id: number, input: ProjectEdit): Promise<void> {
    await updateProject(id, input)
    await load()
  }

  async function remove(id: number): Promise<void> {
    await deleteProject(id)
    await load()
  }

  return { projects, loaded, selected, load, select, update, remove }
})
