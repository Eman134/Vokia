import type { BalloonSource } from './balloonTypes'

export type MomentKind = 'memoria' | 'grafo' | 'decisao' | 'skill' | 'conflito' | 'comemoracao'

export const MOMENT_COPY: Record<MomentKind, { title: string; primary: string; source: BalloonSource }> = {
  memoria: { title: 'Isso parece um fato do projeto', primary: 'Guardar na memória', source: 'conversa' },
  grafo: { title: 'Achei uma ligação nova no código', primary: 'Adicionar ao grafo', source: 'codigo' },
  decisao: { title: 'Isso soa como uma decisão de arquitetura', primary: 'Criar ADR', source: 'chat' },
  skill: { title: 'Vocês pediram isso 3 vezes esta semana', primary: 'Virar skill', source: 'conversa' },
  conflito: { title: 'Dois agentes vão mexer no mesmo arquivo', primary: 'Ordenar no plano', source: 'grafo' },
  comemoracao: { title: 'Lote 2 concluído!', primary: 'Ver o diff', source: 'chat' },
}

export const DISMISS_LABEL = 'Agora não'
