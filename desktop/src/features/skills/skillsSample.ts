import type { PortraitId } from '../team/portraitArt'

export const SKILLS_CONTEXT_LABEL = 'Workspace · 8 skills'
export const SKILLS_TITLE = 'Skills'
export const SKILLS_SUBTITLE =
  'Manuais que o agente segue numa situação. Você liga cada skill às ocupações que precisam dela, e o agente carrega só quando a tarefa pede.'
export const SKILLS_MEMORY_ACTION_LABEL = 'Criar da memória'
export const SKILLS_INSTALL_ACTION_LABEL = 'Instalar skill'
export const SKILL_FROM_MEMORY_LABEL = 'criada da memória'
export const SKILL_MATRIX_TITLE = 'Quem usa o quê'
export const SKILL_MATRIX_SKILL_HEADER = 'Skill'

export interface SkillSample {
  name: string
  description: string
  featured?: boolean
  origin?: string
  fromMemory?: boolean
  tags?: readonly string[]
  boundTo?: string
}

export const SKILL_SAMPLES: readonly SkillSample[] = [
  {
    name: 'Superpowers',
    origin: 'obra/superpowers',
    description:
      'Pensar antes de codar, planejar em etapas, testar primeiro e provar que terminou. Alimenta a tela Plano.',
    featured: true,
    tags: [
      'brainstorm',
      'write-plan',
      'execute-plan',
      'test-driven-development',
      'systematic-debugging',
      'verification-before-completion',
    ],
  },
  {
    name: 'shadcn/ui',
    origin: 'shadcn/skills',
    description: 'Contexto certo dos componentes, dos primitivos Radix e Base UI e do registry do projeto.',
    boundTo: 'Frontend e Designer',
  },
  {
    name: 'pix-loja-online',
    description: 'Gerada de 6 memórias do projeto: txid, expiração do QR, sandbox do PSP e confirmação.',
    fromMemory: true,
    boundTo: 'Backend, Integrações e QA',
  },
]

export interface SkillMatrixOccupation {
  who: PortraitId
  title: string
}

export const SKILL_MATRIX_OCCUPATIONS: readonly SkillMatrixOccupation[] = [
  { who: 'iara', title: 'Tech Lead' },
  { who: 'davi', title: 'Frontend' },
  { who: 'nina', title: 'Backend' },
  { who: 'leo', title: 'Integrações' },
  { who: 'teo', title: 'QA' },
  { who: 'rui', title: 'Pesquisa' },
]

export interface SkillMatrixRow {
  name: string
  uses: readonly boolean[]
}

export const SKILL_MATRIX_ROWS: readonly SkillMatrixRow[] = [
  { name: 'brainstorm', uses: [true, false, false, false, false, true] },
  { name: 'write-plan / execute-plan', uses: [true, false, false, false, false, false] },
  { name: 'test-driven-development', uses: [false, true, true, true, true, false] },
  { name: 'systematic-debugging', uses: [false, true, true, true, true, false] },
  { name: 'verification-before-completion', uses: [true, true, true, true, true, true] },
  { name: 'shadcn/ui', uses: [false, true, false, false, false, false] },
  { name: 'pix-loja-online', uses: [false, false, true, true, true, false] },
]
