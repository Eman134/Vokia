import type UiBadge from '../../ui/UiBadge.vue'

type BadgeStatus = InstanceType<typeof UiBadge>['$props']['status']

export type PlanScope = 'feature' | 'task' | 'bug'

export type PlanStatus = 'run' | 'review' | 'draft' | 'done'
export type ItemState = 'done' | 'now' | 'next'

interface PlanStep {
  name: string
  state: ItemState
  tag: string
  who: string
  model: string
}

interface PlanLot {
  name: string
  state: ItemState
  meta: string
  note: string
  steps: readonly PlanStep[]
  subPlanName?: string
}

export interface SamplePlan {
  id: string
  parentId?: string
  scope: PlanScope
  name: string
  meta: string
  status: PlanStatus
  stage: number
  lots: readonly PlanLot[]
  checks: readonly string[]
}

const CARTAO: SamplePlan = {
  id: 'cartao',
  scope: 'feature',
  name: 'Cartão parcelado',
  meta: 'Iara · 3 lotes · 11 micro-tarefas',
  status: 'run',
  stage: 4,
  lots: [
    { name: 'Lote 1 · Base do cartão', state: 'done', meta: '4 de 4 micro-tarefas', note: 'você revisou às 10:42', steps: [] },
    {
      name: 'Lote 2 · Parcelas no checkout',
      state: 'now',
      meta: '2 de 4 micro-tarefas',
      note: 'revisão no fim do lote',
      steps: [
        { name: 'Escrever teste que falha: 12x com juros', state: 'done', tag: 'TDD', who: 'Léo', model: 'Haiku' },
        { name: 'Implementar calcularParcelas()', state: 'done', tag: 'PAY-13', who: 'Léo', model: 'Sonnet' },
        { name: 'Seletor de parcelas com resumo', state: 'now', tag: 'WEB-21', who: 'Davi', model: 'Sonnet' },
        { name: 'Mensagem para cartão recusado', state: 'next', tag: 'WEB-21', who: 'Davi', model: 'Haiku' },
      ],
      subPlanName: 'PAY-13 · Parcelas e juros',
    },
    { name: 'Lote 3 · Testes ponta a ponta', state: 'next', meta: '3 micro-tarefas', note: 'Téo', steps: [] },
  ],
  checks: ['npm test -- parcelas', 'npx playwright test checkout-cartao', 'npm run typecheck'],
}

const PAY13: SamplePlan = {
  id: 'pay13',
  parentId: 'cartao',
  scope: 'task',
  name: 'PAY-13 · Parcelas e juros',
  meta: 'Léo · 1 lote · 4 micro-tarefas',
  status: 'run',
  stage: 4,
  lots: [
    {
      name: 'Único lote',
      state: 'now',
      meta: '2 de 4 micro-tarefas',
      note: 'Léo',
      steps: [
        { name: 'Teste: 12x com 1,99% a.m.', state: 'done', tag: 'TDD', who: 'Léo', model: 'Haiku' },
        { name: 'calcularParcelas() por bandeira', state: 'done', tag: 'src/pay', who: 'Léo', model: 'Sonnet' },
        { name: 'Arredondamento na última parcela', state: 'now', tag: 'RN-04', who: 'Léo', model: 'Haiku' },
        { name: 'Pedir revisão ao Téo', state: 'next', tag: '', who: 'Léo', model: 'Sonnet' },
      ],
    },
  ],
  checks: ['npm test -- parcelas'],
}

const WEB21: SamplePlan = {
  id: 'web21',
  parentId: 'cartao',
  scope: 'task',
  name: 'WEB-21 · Seletor de parcelas',
  meta: 'Davi · rascunho do Davi',
  status: 'draft',
  stage: 3,
  lots: [
    {
      name: 'Único lote',
      state: 'next',
      meta: '3 micro-tarefas',
      note: 'Davi',
      steps: [
        { name: 'Componente de seleção', state: 'next', tag: 'WEB-21', who: 'Davi', model: 'Sonnet' },
        { name: 'Resumo do total', state: 'next', tag: 'WEB-21', who: 'Davi', model: 'Haiku' },
        { name: 'Acessibilidade do seletor', state: 'next', tag: 'WEB-21', who: 'Davi', model: 'Haiku' },
      ],
    },
  ],
  checks: ['npm test -- seletor'],
}

const PIX: SamplePlan = {
  id: 'pix',
  scope: 'feature',
  name: 'Pagamento via Pix',
  meta: 'Iara · 3 lotes · 12 micro-tarefas',
  status: 'review',
  stage: 5,
  lots: [
    { name: 'Lote 1 · API de cobrança', state: 'done', meta: '4 de 4', note: 'Nina', steps: [] },
    { name: 'Lote 2 · Tela do QR', state: 'done', meta: '4 de 4', note: 'Davi', steps: [] },
    {
      name: 'Lote 3 · Webhook e confirmação',
      state: 'now',
      meta: '3 de 4',
      note: 'Téo revisando',
      steps: [
        { name: 'Webhook idempotente por txid', state: 'done', tag: 'PIX-2', who: 'Nina', model: 'Sonnet' },
        { name: 'Confirmar sem recarregar', state: 'done', tag: 'PIX-4', who: 'Davi', model: 'Sonnet' },
        { name: 'Aviso de QR expirado', state: 'now', tag: 'PIX-4', who: 'Davi', model: 'Haiku' },
        { name: 'Revisão final', state: 'next', tag: '', who: 'Téo', model: 'Sonnet' },
      ],
    },
  ],
  checks: ['npm test -- pix', 'npx playwright test checkout-pix'],
}

const BUG7: SamplePlan = {
  id: 'bug7',
  scope: 'bug',
  name: 'BUG-7 · Total arredondado errado',
  meta: 'Léo · 1 lote · 2 micro-tarefas',
  status: 'done',
  stage: 6,
  lots: [{ name: 'Único lote', state: 'done', meta: '2 de 2', note: 'concluído ontem', steps: [] }],
  checks: ['npm test -- total'],
}

export const PLANS: readonly SamplePlan[] = [CARTAO, PAY13, WEB21, PIX, BUG7]
export const INITIAL_PLAN = CARTAO

export const PLAN_GROUPS: readonly { scope: PlanScope; label: string }[] = [
  { scope: 'feature', label: 'POR FEATURE' },
  { scope: 'task', label: 'POR TASK' },
  { scope: 'bug', label: 'POR BUG' },
]

export const PLAN_STAGES: readonly { name: string; sub: string }[] = [
  { name: 'Brainstorm', sub: 'perguntas respondidas' },
  { name: 'Especificação', sub: 'aprovada' },
  { name: 'Plano', sub: 'lotes e micro-tarefas' },
  { name: 'Execução em lotes', sub: 'em andamento' },
  { name: 'Verificação', sub: 'testes e revisão' },
  { name: 'Entrega', sub: 'PR para main' },
]

export const PLAN_STATUS_LABEL: Record<PlanStatus, string> = {
  run: 'executando',
  review: 'em revisão',
  draft: 'rascunho',
  done: 'concluído',
}
export const PLAN_STATUS_BADGE: Record<PlanStatus, BadgeStatus> = {
  run: 'trabalhando',
  review: 'lote',
  draft: 'backlog',
  done: 'livre',
}

export const ITEM_STATE_LABEL: Record<ItemState, string> = {
  done: 'feito',
  now: 'em andamento',
  next: 'a fazer',
}
export const ITEM_STATE_BADGE: Record<ItemState, BadgeStatus> = {
  done: 'livre',
  now: 'trabalhando',
  next: 'backlog',
}

export const PLAN_LIST_TITLE = 'Planos do projeto'
export const NEW_PLAN_LABEL = '+ Novo plano'
export const PLAN_FILES: readonly string[] = ['PLAN.md', 'progress.md', 'verification.md']
export const SCOPE_CRUMB_PREFIX = 'plano de'
export const SCOPE_SUBTITLE_PREFIX = 'Plano de'
export const SUB_PLAN_SUBTITLE_PREFIX = 'Sub-plano de uma task dentro de'
export const PLAN_STORAGE_TEXT = 'Fica em arquivos no worktree, então o trabalho continua mesmo se a sessão cair.'
