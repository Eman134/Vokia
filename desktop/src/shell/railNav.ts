export type RailPage =
  | 'projetos'
  | 'agentes'
  | 'estatisticas'
  | 'skills'
  | 'mapa'
  | 'backlog'
  | 'conversas'
  | 'plano'
  | 'canvas'
  | 'diff'
  | 'memoria'
  | 'config'

export type AppPage = RailPage | 'novo-projeto' | 'onboarding'

export interface RailItem {
  page: RailPage
  label: string
  path: string
}

export interface RailProject {
  name: string
  working: number
  dot: 'warn' | 'ok'
  path: string
  page: RailPage
}

export const PAGE_LABELS: Record<AppPage, string> = {
  projetos: 'Projetos',
  agentes: 'Agentes e camadas',
  estatisticas: 'Estatísticas',
  skills: 'Skills',
  mapa: 'Mapa do time',
  backlog: 'Backlog e planos',
  conversas: 'Conversas',
  plano: 'Plano',
  canvas: 'Artefatos',
  diff: 'Diff e revisão',
  memoria: 'Memória e grafo',
  config: 'Configurações',
  'novo-projeto': 'Novo projeto',
  onboarding: 'Onboarding',
}

export const DEFAULT_PAGE: AppPage = 'conversas'

export const SAMPLE_DATA_LABEL = 'dados de exemplo'

export const CURRENT_PROJECT = 'loja-online'

export const RAIL_COLLAPSED_WIDTH = 88
export const RAIL_EXPANDED_WIDTH = 232

export const WORKSPACE_ITEMS: readonly RailItem[] = [
  {
    page: 'projetos',
    label: PAGE_LABELS.projetos,
    path: 'M2 2.5h5v5H2zM9 2.5h5v5H9zM2 9.5h5v4H2zM9 9.5h5v4H9z',
  },
  {
    page: 'agentes',
    label: PAGE_LABELS.agentes,
    path: 'M6 7.5a2.5 2.5 0 100-5 2.5 2.5 0 000 5zM1.5 14c0-2.6 2-4.2 4.5-4.2s4.5 1.6 4.5 4.2M11 3a2.2 2.2 0 010 4.3M12.3 9.9c1.4.5 2.2 1.9 2.2 4.1',
  },
  {
    page: 'estatisticas',
    label: PAGE_LABELS.estatisticas,
    path: 'M2 13.5h12M4 11V8M7 11V4.5M10 11V6.5M13 11V3',
  },
  {
    page: 'skills',
    label: PAGE_LABELS.skills,
    path: 'M9 1.5L3.5 9h4L6.5 14.5 12.5 6.5h-4z',
  },
]

export const PROJECT_TOOLS: readonly RailItem[] = [
  {
    page: 'mapa',
    label: PAGE_LABELS.mapa,
    path: 'M1.5 3.5h13v9h-13zM1.5 6.5h13M5.5 3.5v9M10 3.5v9',
  },
  {
    page: 'backlog',
    label: PAGE_LABELS.backlog,
    path: 'M2 2.5h3.5v11H2zM6.25 2.5h3.5v7h-3.5zM10.5 2.5H14v4.5h-3.5z',
  },
  {
    page: 'conversas',
    label: PAGE_LABELS.conversas,
    path: 'M1.5 3h8.5v6H5l-3.5 2.5zM12 6h2.5v6.5L12 11H7.5V9',
  },
  {
    page: 'plano',
    label: PAGE_LABELS.plano,
    path: 'M3 3h2M3 8h2M3 13h2M7.5 3h6M7.5 8h6M7.5 13h6',
  },
  {
    page: 'canvas',
    label: PAGE_LABELS.canvas,
    path: 'M2 2.5h5v4H2zM9.5 9.5a2.5 2.5 0 105 0 2.5 2.5 0 00-5 0zM2 13.5l3-4 3 4zM7 4.5h3.5v3',
  },
  {
    page: 'diff',
    label: PAGE_LABELS.diff,
    path: 'M4 2v8M4 12.5a1.5 1.5 0 100-.1M12 3.5a1.5 1.5 0 100-.1M12 5v2c0 2-2 3-5.5 3.5',
  },
  {
    page: 'memoria',
    label: PAGE_LABELS.memoria,
    path: 'M4 4a2 2 0 100-.1M12 4a2 2 0 100-.1M8 12a2 2 0 100-.1M5.5 5.5L7 10.5M10.5 5.5L9 10.5M6 4h4',
  },
]

export const PROJECTS: readonly RailProject[] = [
  {
    name: 'loja-online',
    working: 5,
    dot: 'warn',
    path: 'M9 2.5a6.5 6.5 0 110 13 6.5 6.5 0 010-13zM9 7a2 2 0 110 4 2 2 0 010-4z',
    page: 'projetos',
  },
  {
    name: 'app-agenda',
    working: 2,
    dot: 'warn',
    path: 'M9 3l6.5 12h-13z',
    page: 'projetos',
  },
  {
    name: 'site-estudio',
    working: 2,
    dot: 'ok',
    path: 'M3.5 3.5h11v11h-11z',
    page: 'projetos',
  },
]
