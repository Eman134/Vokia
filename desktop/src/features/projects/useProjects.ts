import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { listProjects, selectProject, type Project } from './projectsApi'

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

  return { projects, loaded, selected, load, select }
})
