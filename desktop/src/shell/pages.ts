import type { Component } from 'vue'
import AgentList from '../features/team/AgentList.vue'
import Conversation from '../features/team/Conversation.vue'
import SkillsPage from '../features/skills/SkillsPage.vue'
import ArtefatosPage from '../features/artifacts/ArtefatosPage.vue'
import ConfiguracoesPage from '../features/settings/ConfiguracoesPage.vue'
import OnboardingPage from '../features/onboarding/OnboardingPage.vue'
import MemoriaPage from '../features/memory/MemoriaPage.vue'
import PlanosPage from '../features/plans/PlanosPage.vue'
import type { AppPage } from './railNav'

export const PAGE_VIEWS: Partial<Record<AppPage, readonly Component[]>> = {
  conversas: [AgentList, Conversation],
  skills: [SkillsPage],
  canvas: [ArtefatosPage],
  config: [ConfiguracoesPage],
  onboarding: [OnboardingPage],
  memoria: [MemoriaPage],
  plano: [PlanosPage],
}
