import type { BacklogColumn, BacklogItem } from './backlogTypes'

export const BACKLOG_TITLE = 'Backlog e planos'
export const BACKLOG_SUBTITLE =
  'Todo pedido entra aqui. Features viram um card pai: a Iara faz a triagem e desdobra em micro-tarefas, o time executa, você revisa e elas são entregues.'
export const NEW_REQUEST_LABEL = '+ Novo pedido'
export const PLAN_ALL_LABEL = 'Triar os {count} pendentes'
export const ALL_TRIAGED_LABEL = 'Tudo triado'
export const SEARCH_LABEL = 'Buscar no backlog'
export const SEARCH_PLACEHOLDER = 'Buscar item'
export const MINE_LABEL = 'Meus pedidos'
export const EMPTY_COLUMN_TEXT = 'Nada aqui'

export const DENSITY_OPTIONS = [
  { label: 'Normal', value: 'normal' },
  { label: 'Compacta', value: 'compacta' },
] as const
export const FEATURE_FILTER_OPTIONS = [
  { label: 'Todas', value: 'todas' },
  { label: 'Pix', value: 'Pagamento via Pix' },
  { label: 'Painel', value: 'Painel administrativo' },
  { label: 'Checkout', value: 'Checkout' },
  { label: 'Cartão', value: 'Cartão parcelado' },
] as const
export const TYPE_FILTER_OPTIONS = [
  { label: 'Tudo', value: 'tudo' },
  { label: 'Features', value: 'feature' },
  { label: 'Micro-tarefas', value: 'micro' },
] as const

export interface ColumnSample {
  name: BacklogColumn
  hint: string
  dotToken: string
}

export const COLUMN_SAMPLES: readonly ColumnSample[] = [
  { name: 'Backlog', hint: 'pedidos e ideias; passam pela triagem aqui', dotToken: 'faint' },
  { name: 'Ready', hint: 'plano aprovado, esperando o time', dotToken: 'accent' },
  { name: 'In progress', hint: 'um agente está trabalhando', dotToken: 'accent' },
  { name: 'In review', hint: 'diff esperando revisão', dotToken: 'warn' },
  { name: 'Done', hint: 'entregue nesta semana', dotToken: 'ok' },
]

export const SAMPLE_ITEMS: readonly BacklogItem[] = [
  {
    id: 'FRT-1',
    title: 'Frete grátis por região',
    type: 'feature',
    state: 'triagem',
    triageStep: 2,
    feature: 'Checkout',
    tags: ['cliente'],
    description: 'Frete grátis acima de um valor mínimo por estado, com aviso no carrinho.',
    plan: 'Iara · 1 pergunta para você',
    who: 'voce',
    by: 'você · agora',
    questions: [
      {
        id: 'frt-1-q1',
        text: 'O valor mínimo é por estado ou por região (Sul, Sudeste…)?',
        options: ['Por estado', 'Por região'],
      },
      { id: 'frt-1-q2', text: 'O frete grátis acumula com cupom?', options: ['Acumula', 'Não acumula'] },
    ],
  },
  {
    id: 'ADM-02',
    title: 'Painel de pedidos',
    type: 'task',
    state: 'vazio',
    feature: 'Painel administrativo',
    tags: ['cliente'],
    description: 'Listar pedidos com filtro por status e busca por cliente. Precisa do status do Pix.',
    who: 'voce',
    by: 'você · ontem',
  },
  {
    id: 'WEB-24',
    title: 'Modo escuro no checkout',
    type: 'task',
    state: 'vazio',
    feature: 'Checkout',
    tags: ['ui', 'ideia'],
    description: 'Usar os tokens do design system no checkout.',
    who: 'Bia',
    by: 'Bia · 3 dias',
  },
  {
    id: 'ADM-03',
    title: 'Exportar pedidos em CSV',
    type: 'task',
    state: 'vazio',
    feature: 'Painel administrativo',
    tags: [],
    description: 'Botão de exportar na lista do painel.',
    who: 'voce',
    by: 'você · semana passada',
  },
  {
    id: 'PAY-14',
    title: 'Reembolso parcial',
    type: 'feature',
    state: 'proposto',
    feature: 'Cartão parcelado',
    tags: ['backend', 'urgente'],
    description: 'Devolver parte do valor de um pedido pago no cartão.',
    plan: '4 micro-tarefas · ~2 h',
    who: 'Téo',
    by: 'Téo · hoje',
  },
  {
    id: 'ADM-04',
    title: 'Avisar pedido parado',
    type: 'task',
    state: 'triagem',
    triageStep: 1,
    feature: 'Painel administrativo',
    tags: ['backend'],
    description: 'Notificar quando um pedido fica 24 h sem mudar de status.',
    plan: 'Rui · pesquisando para a Iara',
    who: 'Nina',
    by: 'Nina · hoje',
  },
  {
    id: 'PIX-6',
    title: 'Cupom de desconto no Pix',
    type: 'task',
    state: 'proposto',
    feature: 'Pagamento via Pix',
    tags: ['ui', 'cliente'],
    description: 'Aplicar cupom antes de gerar a cobrança. Iara já propôs um plano.',
    plan: '2 lotes · ~4 h',
    who: 'Iara',
    by: 'Iara · hoje',
  },
  {
    id: 'PAY-15',
    title: 'Parcelar sem juros',
    type: 'task',
    state: 'proposto',
    feature: 'Cartão parcelado',
    tags: ['ideia'],
    description: 'Até 3x sem juros acima de R$ 300.',
    plan: '3 lotes · ~6 h',
    who: 'voce',
    by: 'você · hoje',
  },
  {
    id: 'PIX-7',
    title: 'Comprovante por e-mail',
    type: 'task',
    state: 'vinculado',
    stage: 'ready',
    feature: 'Pagamento via Pix',
    tags: ['backend'],
    description: 'Enviar o comprovante quando o webhook confirmar.',
    plan: 'Plano · Pagamento via Pix',
    who: 'voce',
    by: 'você · 2 dias',
  },
  {
    id: 'CHK-09',
    title: 'Cupom por link',
    type: 'task',
    state: 'vinculado',
    stage: 'ready',
    feature: 'Checkout',
    tags: ['bug', 'urgente'],
    description: 'Aplicar o cupom vindo de ?cupom= na URL.',
    plan: 'Plano · Checkout',
    who: 'Davi',
    by: 'Davi · 4 dias',
  },
  {
    id: 'PIX',
    title: 'Pagamento via Pix',
    type: 'feature',
    state: 'vinculado',
    stage: 'progress',
    feature: 'Pagamento via Pix',
    tags: ['backend'],
    description: 'Cobrança, QR Code, webhook e comprovante.',
    children: { total: 4, done: 1 },
    who: 'voce',
    by: 'você · semana passada',
  },
  {
    id: 'PIX-3',
    title: 'QR e expiração',
    type: 'task',
    state: 'vinculado',
    stage: 'progress',
    feature: 'Pagamento via Pix',
    tags: ['backend'],
    description: 'Gerar o QR Code e expirar a cobrança em 30 min.',
    plan: 'Lote 2 de 3',
    who: 'Davi',
    by: 'Davi',
  },
  {
    id: 'PAY-12',
    title: 'Gateway de cartão',
    type: 'task',
    state: 'vinculado',
    stage: 'progress',
    feature: 'Cartão parcelado',
    tags: ['backend', 'urgente'],
    description: 'Integrar o gateway e tokenizar o cartão.',
    plan: 'Lote 1 de 2',
    who: 'Nina',
    by: 'Nina',
  },
  {
    id: 'PIX-4',
    title: 'Webhook do PSP',
    type: 'task',
    state: 'vinculado',
    stage: 'review',
    feature: 'Pagamento via Pix',
    tags: ['backend'],
    description: 'Receber a confirmação do PSP e marcar o pedido como pago.',
    plan: 'Téo revisando · 14 arquivos',
    who: 'Nina',
    by: 'Nina · diff pronto',
  },
  {
    id: 'PIX-2',
    title: 'Cobrança no PSP',
    type: 'task',
    state: 'vinculado',
    stage: 'done',
    feature: 'Pagamento via Pix',
    tags: ['backend'],
    description: 'Criar a cobrança Pix no PSP a partir do pedido.',
    plan: 'Entregue · 3 lotes',
    who: 'Davi',
    by: 'Davi · ontem',
  },
  {
    id: 'CHK-05',
    title: 'Resumo do pedido',
    type: 'task',
    state: 'vinculado',
    stage: 'done',
    feature: 'Checkout',
    tags: ['ui'],
    description: 'Mostrar itens, frete e total antes de pagar.',
    plan: 'Entregue · 1 lote',
    who: 'Bia',
    by: 'Bia · 2 dias',
  },
]

export const SAMPLE_IDS: ReadonlySet<string> = new Set(SAMPLE_ITEMS.map((item) => item.id))
