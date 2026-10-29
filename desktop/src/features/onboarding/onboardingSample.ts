export type CliStatus = 'connected' | 'pending'

export interface CliSample {
  abbr: string
  name: string
  status: CliStatus
  statusLabel: string
}

export const CLI_SAMPLES: readonly CliSample[] = [
  { abbr: 'CC', name: 'Claude Code', status: 'connected', statusLabel: 'Logado' },
  { abbr: 'CX', name: 'Codex', status: 'pending', statusLabel: 'Instalado, falta entrar' },
]

export interface RepoSample {
  name: string
  detail: string
  selected: boolean
}

export const REPO_SAMPLES: readonly RepoSample[] = [
  { name: 'kayke/loja-online', detail: 'TypeScript · atualizado ontem', selected: true },
  { name: 'kayke/app-agenda', detail: 'React Native · há 3 dias', selected: false },
  { name: 'kayke/vokia', detail: 'TypeScript · há 1 hora', selected: false },
]

export const BRANCH_SAMPLE = 'develop'

export interface TeamSample {
  name: string
  description: string
  members: readonly string[]
  selected: boolean
}

export const TEAM_SAMPLES: readonly TeamSample[] = [
  {
    name: 'Web',
    description: 'liderança, front, back e revisão',
    members: ['iara', 'davi', 'nina', 'teo'],
    selected: true,
  },
  { name: 'Pesquisa', description: 'pesquisar, comparar e resumir', members: ['rui', 'iara'], selected: false },
  { name: 'Dupla', description: 'uma líder e quem implementa', members: ['iara', 'davi'], selected: false },
]

export interface RequestSample {
  label: string
  text: string
  selected: boolean
}

export const REQUEST_SAMPLES: readonly RequestSample[] = [
  {
    label: 'Pagamento via Pix',
    text: 'Adicionar pagamento via Pix no checkout: QR Code que expira em 15 minutos, confirmação pelo webhook do PSP e tela de pedido pago.',
    selected: true,
  },
  {
    label: 'Mapear o código',
    text: 'Ler o repositório e montar um mapa das telas, serviços e dependências, com os pontos frágeis.',
    selected: false,
  },
  {
    label: 'Resolver issues',
    text: 'Resolver as issues abertas marcadas como bug, uma feature por issue, começando pela mais simples.',
    selected: false,
  },
]

export interface TipSample {
  key: string
  text: string
}

export const FINAL_TIPS: readonly TipSample[] = [
  { key: 'barra', text: 'A barra de baixo mostra o progresso e as decisões que esperam você' },
  { key: '@nome', text: 'Fale direto com um agente no campo da barra' },
  { key: '⌘K', text: 'Procure páginas, cards e memória' },
]
