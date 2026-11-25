import type { Component } from 'vue'
import ConversasPage from '../features/team/ConversasPage.vue'
import AgentesPage from '../features/agents/AgentesPage.vue'
import SkillsPage from '../features/skills/SkillsPage.vue'
import ArtefatosPage from '../features/artifacts/ArtefatosPage.vue'
import ConfiguracoesPage from '../features/settings/ConfiguracoesPage.vue'
import OnboardingPage from '../features/onboarding/OnboardingPage.vue'
import MemoriaPage from '../features/memory/MemoriaPage.vue'
import ProjetosPage from '../features/projects/ProjetosPage.vue'
import NovoProjetoPage from '../features/projects/NovoProjetoPage.vue'
import TasksPage from '../features/backlog/TasksPage.vue'
import MapaPage from '../features/teammap/MapaPage.vue'
import EstatisticasPage from '../features/usage/EstatisticasPage.vue'
import type { AppPage } from './railNav'

export const PAGE_VIEWS: Partial<Record<AppPage, readonly Component[]>> = {
  conversas: [ConversasPage],
  agentes: [AgentesPage],
  estatisticas: [EstatisticasPage],
  skills: [SkillsPage],
  canvas: [ArtefatosPage],
  config: [ConfiguracoesPage],
  onboarding: [OnboardingPage],
  memoria: [MemoriaPage],
  projetos: [ProjetosPage],
  'novo-projeto': [NovoProjetoPage],
  backlog: [TasksPage],
  mapa: [MapaPage],
}
