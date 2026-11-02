import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { Project } from '../features/projects/projectsApi'
import { useProjects } from '../features/projects/useProjects'
import { DEFAULT_PAGE, isProjectPage, type AppPage } from './railNav'

export const useNavigation = defineStore('navigation', () => {
  const page = ref<AppPage>(DEFAULT_PAGE)
  const projects = useProjects()
  // Só na sessão: a lembrança não é persistida no banco.
  const lastPageByProject = new Map<number, AppPage>()

  function rememberedPage(projectId: number): AppPage {
    return lastPageByProject.get(projectId) ?? DEFAULT_PAGE
  }

  function goTo(target: AppPage): void {
    const selected = projects.selected
    if (selected && isProjectPage(target)) {
      lastPageByProject.set(selected.id, target)
    }
    page.value = target
  }

  async function enterProject(project: Project): Promise<void> {
    await projects.select(project.id)
    // Outra seleção começou depois desta: quem manda na aba é a última.
    if (projects.selected?.id !== project.id) return
    goTo(rememberedPage(project.id))
  }

  return { page, goTo, rememberedPage, enterProject }
})
