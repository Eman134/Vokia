import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import {
  deleteProject,
  listProjects,
  pinProject,
  selectProject,
  updateProject,
  type Project,
  type ProjectEdit,
} from './projectsApi'

export const useProjects = defineStore('projects', () => {
  const projects = ref<Project[]>([])
  const loaded = ref(false)
  const selected = computed(() => projects.value.find((project) => project.selected) ?? null)
  const pinned = computed(() => projects.value.find((project) => project.pinned) ?? null)

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

  async function pin(id: number | null): Promise<void> {
    await pinProject(id)
    await load()
  }

  return { projects, loaded, selected, pinned, load, select, update, remove, pin }
})
