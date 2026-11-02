import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import type { Project } from '../features/projects/projectsApi'
import { useProjects } from '../features/projects/useProjects'
import { DEFAULT_PAGE, PROJECT_LIST_PAGE, isProjectPage, type AppPage } from './railNav'

export const useNavigation = defineStore('navigation', () => {
  const page = ref<AppPage>(DEFAULT_PAGE)
  const projects = useProjects()
  // Só na sessão: a lembrança não é persistida no banco.
  const lastPageByProject = new Map<number, AppPage>()
  const hasNoProject = computed(() => projects.loaded && projects.selected === null)

  // Cobre o início sem projeto e a exclusão do último, quando a página atual já é de projeto.
  watch(hasNoProject, (noProject) => {
    if (noProject && isProjectPage(page.value)) page.value = PROJECT_LIST_PAGE
  })

  function rememberedPage(projectId: number): AppPage {
    return lastPageByProject.get(projectId) ?? DEFAULT_PAGE
  }

  function goTo(target: AppPage): void {
    const selected = projects.selected
    if (selected && isProjectPage(target)) {
      lastPageByProject.set(selected.id, target)
    }
    page.value = selected === null && isProjectPage(target) ? PROJECT_LIST_PAGE : target
  }

  async function enterProject(project: Project): Promise<void> {
    await projects.select(project.id)
    // Outra seleção começou depois desta: quem manda na aba é a última.
    if (projects.selected?.id !== project.id) return
    goTo(rememberedPage(project.id))
  }

  return { page, goTo, rememberedPage, enterProject }
})
