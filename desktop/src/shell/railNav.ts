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

export interface RailItem {
  id: RailPage
  label: string
  path: string
}

export interface RailProject {
  name: string
  working: number
  dot: 'warn' | 'ok'
  path: string
}

export const ACTIVE_PAGE: RailPage = 'conversas'

export const CURRENT_PROJECT = 'loja-online'

export const RAIL_COLLAPSED_WIDTH = 88
export const RAIL_EXPANDED_WIDTH = 232

export const WORKSPACE_ITEMS: readonly RailItem[] = [
  {
    id: 'projetos',
    label: 'Projetos',
    path: 'M2 2.5h5v5H2zM9 2.5h5v5H9zM2 9.5h5v4H2zM9 9.5h5v4H9z',
  },
  {
    id: 'agentes',
    label: 'Agentes e camadas',
    path: 'M6 7.5a2.5 2.5 0 100-5 2.5 2.5 0 000 5zM1.5 14c0-2.6 2-4.2 4.5-4.2s4.5 1.6 4.5 4.2M11 3a2.2 2.2 0 010 4.3M12.3 9.9c1.4.5 2.2 1.9 2.2 4.1',
  },
  {
    id: 'estatisticas',
    label: 'Estatísticas',
    path: 'M2 13.5h12M4 11V8M7 11V4.5M10 11V6.5M13 11V3',
  },
  {
    id: 'skills',
    label: 'Skills',
    path: 'M9 1.5L3.5 9h4L6.5 14.5 12.5 6.5h-4z',
  },
]

export const PROJECT_TOOLS: readonly RailItem[] = [
  {
    id: 'mapa',
    label: 'Mapa do time',
    path: 'M1.5 3.5h13v9h-13zM1.5 6.5h13M5.5 3.5v9M10 3.5v9',
  },
  {
    id: 'backlog',
    label: 'Backlog e planos',
    path: 'M2 2.5h3.5v11H2zM6.25 2.5h3.5v7h-3.5zM10.5 2.5H14v4.5h-3.5z',
  },
  {
    id: 'conversas',
    label: 'Conversas',
    path: 'M1.5 3h8.5v6H5l-3.5 2.5zM12 6h2.5v6.5L12 11H7.5V9',
  },
  {
    id: 'plano',
    label: 'Plano',
    path: 'M3 3h2M3 8h2M3 13h2M7.5 3h6M7.5 8h6M7.5 13h6',
  },
  {
    id: 'canvas',
    label: 'Artefatos',
    path: 'M2 2.5h5v4H2zM9.5 9.5a2.5 2.5 0 105 0 2.5 2.5 0 00-5 0zM2 13.5l3-4 3 4zM7 4.5h3.5v3',
  },
  {
    id: 'diff',
    label: 'Diff e revisão',
    path: 'M4 2v8M4 12.5a1.5 1.5 0 100-.1M12 3.5a1.5 1.5 0 100-.1M12 5v2c0 2-2 3-5.5 3.5',
  },
  {
    id: 'memoria',
    label: 'Memória e grafo',
    path: 'M4 4a2 2 0 100-.1M12 4a2 2 0 100-.1M8 12a2 2 0 100-.1M5.5 5.5L7 10.5M10.5 5.5L9 10.5M6 4h4',
  },
]

export const PROJECTS: readonly RailProject[] = [
  {
    name: 'loja-online',
    working: 5,
    dot: 'warn',
    path: 'M9 2.5a6.5 6.5 0 110 13 6.5 6.5 0 010-13zM9 7a2 2 0 110 4 2 2 0 010-4z',
  },
  {
    name: 'app-agenda',
    working: 2,
    dot: 'warn',
    path: 'M9 3l6.5 12h-13z',
  },
  {
    name: 'site-estudio',
    working: 2,
    dot: 'ok',
    path: 'M3.5 3.5h11v11h-11z',
  },
]
