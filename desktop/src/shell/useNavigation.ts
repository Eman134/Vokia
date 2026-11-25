import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import type { Project } from '../features/projects/projectsApi'
import { useProjects } from '../features/projects/useProjects'
import { useUserProfile } from '../features/profile/useUserProfile'
import { DEFAULT_PAGE, PROJECT_LIST_PAGE, isProjectPage, type AppPage } from './railNav'

export interface PendingMessageTarget {
  time: number
  from: string
  text: string
  channelId?: number | null
  itemId?: string | null
}

export const useNavigation = defineStore('navigation', () => {
  const profile = useUserProfile()
  // Perfil incompleto abre o onboarding antes do resto do app; main.ts carrega o perfil antes de montar.
  const page = ref<AppPage>(profile.pendingStep === null ? DEFAULT_PAGE : 'onboarding')
  const pendingArtifactPlanId = ref<string | null>(null)
  const pendingArtifactDocumentId = ref<string | null>(null)
  const pendingSettingsSection = ref<string | null>(null)
  const pendingCardId = ref<string | null>(null)
  const pendingMessageTarget = ref<PendingMessageTarget | null>(null)
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

  function openPlanArtifact(itemId: string): void {
    pendingArtifactPlanId.value = itemId
    goTo('canvas')
  }

  function takePendingPlanId(): string | null {
    const id = pendingArtifactPlanId.value
    pendingArtifactPlanId.value = null
    return id
  }

  function openArtifactDocument(artifactId: string): void {
    pendingArtifactDocumentId.value = artifactId
    goTo('canvas')
  }

  function takePendingArtifactDocumentId(): string | null {
    const id = pendingArtifactDocumentId.value
    pendingArtifactDocumentId.value = null
    return id
  }

  function openSettingsSection(sectionTitle: string): void {
    pendingSettingsSection.value = sectionTitle
    goTo('config')
  }

  function takePendingSettingsSection(): string | null {
    const section = pendingSettingsSection.value
    pendingSettingsSection.value = null
    return section
  }

  function openCard(itemId: string): void {
    pendingCardId.value = itemId
    goTo('backlog')
  }

  function takePendingCardId(): string | null {
    const id = pendingCardId.value
    pendingCardId.value = null
    return id
  }

  function openMessage(target: PendingMessageTarget): void {
    pendingMessageTarget.value = target
    goTo('conversas')
  }

  function takePendingMessageTarget(): PendingMessageTarget | null {
    const target = pendingMessageTarget.value
    pendingMessageTarget.value = null
    return target
  }

  return {
    page,
    pendingArtifactPlanId,
    pendingArtifactDocumentId,
    pendingSettingsSection,
    pendingCardId,
    pendingMessageTarget,
    goTo,
    openPlanArtifact,
    takePendingPlanId,
    openArtifactDocument,
    takePendingArtifactDocumentId,
    openSettingsSection,
    takePendingSettingsSection,
    openCard,
    takePendingCardId,
    openMessage,
    takePendingMessageTarget,
    rememberedPage,
    enterProject,
  }
})
