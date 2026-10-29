import type { RailPage } from '../../shell/railNav'

export type ComposeSize = 'feature'

export interface PageAction {
  id: string
  label: string
  size?: ComposeSize
}

export const ACTION_SHORTCUT_MODIFIER = 'Alt'

export const PAGE_ACTIONS: Record<RailPage, PageAction[]> = {
  projetos: [{ id: 'novo-projeto', label: 'Novo projeto' }],
  mapa: [
    { id: 'pausar-time', label: 'Pausar time' },
    { id: 'nova-feature', label: 'Nova feature', size: 'feature' },
  ],
  backlog: [
    { id: 'novo-pedido', label: 'Novo pedido', size: 'feature' },
    { id: 'triar-pendentes', label: 'Triar pendentes' },
  ],
  conversas: [
    { id: 'nova-conversa', label: 'Nova conversa' },
    { id: 'pausar-todos', label: 'Pausar todos' },
  ],
  plano: [
    { id: 'revisar-lote', label: 'Revisar lote' },
    { id: 'pausar-plano', label: 'Pausar plano' },
  ],
  canvas: [
    { id: 'novo-artefato', label: 'Novo artefato' },
    { id: 'gerar-cards', label: 'Gerar cards' },
  ],
  memoria: [
    { id: 'revisar-fatos', label: 'Revisar fatos' },
    { id: 'buscar-na-memoria', label: 'Buscar na memória' },
  ],
  agentes: [
    { id: 'novo-agente', label: 'Novo agente' },
    { id: 'enhance', label: 'Enhance' },
  ],
  estatisticas: [{ id: 'exportar', label: 'Exportar' }],
  skills: [
    { id: 'nova-skill', label: 'Nova skill' },
    { id: 'buscar-skill', label: 'Buscar skill' },
  ],
  diff: [
    { id: 'pedir-correcao', label: 'Pedir correção' },
    { id: 'abrir-pr', label: 'Abrir PR' },
  ],
  config: [{ id: 'verificar-clis', label: 'Verificar CLIs' }],
}
