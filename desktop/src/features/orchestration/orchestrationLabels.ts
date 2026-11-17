import type { Agent } from '../team/events'
import {
  VOCE_NODE,
  type ExecMode,
  type HumanGate,
  type LayerKind,
  type OrchAction,
  type OrchMovement,
  type OrchRule,
  type RequestKind,
  type StepMode,
  type ThenChoice,
  type WhenEvent,
  type WorktreePolicy,
} from './orchestrationTypes'

export const LAYER_KIND_LABELS: Record<LayerKind, string> = {
  planejar: 'Planejar',
  implementar: 'Implementar',
  revisar: 'Revisar',
  pesquisar: 'Pesquisar',
  livre: 'Livre',
}

export const WHEN_EVENT_PHRASES: Record<WhenEvent, string> = {
  plano_aprovado: 'o plano for aprovado',
  tarefa_concluida: 'a tarefa for concluída',
  revisao_reprovou: 'a revisão reprovar',
  testes_falharam: 'os testes falharem',
  revisao_aprovou: 'a revisão aprovar',
  pedido_de_ajuda: 'o agente pedir ajuda',
}

export const ACTION_SYMBOLS: Record<OrchAction, string> = {
  delegar: '→',
  devolver: '↩',
  escalar: '⇢',
}

export const VOCE_LABEL = 'Você'
const LOCALE = 'pt-BR'
const MOVEMENT_TIME_OPTIONS: Intl.DateTimeFormatOptions = { hour: '2-digit', minute: '2-digit' }

const REQUEST_VERBS: Record<Exclude<RequestKind, 'implementar'>, string> = {
  planejar: 'pediu plano',
  revisar: 'pediu revisão',
  pesquisar: 'pediu pesquisa',
  testar: 'pediu testes',
}

export function layerName(layers: readonly { id: string; name: string }[], id: string): string {
  return layers.find((layer) => layer.id === id)?.name ?? id
}

export function ruleLine(rule: OrchRule, layers: readonly { id: string; name: string }[]): string {
  const target = rule.toLayer === VOCE_NODE ? VOCE_LABEL : layerName(layers, rule.toLayer)
  const origin = layerName(layers, rule.fromLayer)
  return `${origin} ${ACTION_SYMBOLS[rule.action]} ${target} · quando ${WHEN_EVENT_PHRASES[rule.whenEvent]}`
}

export function movementVerb(
  movement: OrchMovement,
  rules: readonly OrchRule[],
  nameOf: (id: Agent) => string,
): string {
  const target = nameOf(movement.toAgent)
  const item = movement.itemId
  if (movement.action === 'escalar') {
    return `chamou você${item ? ` sobre ${item}` : ''}`
  }
  if (movement.action === 'devolver') {
    const rule = rules.find(
      (candidate) => candidate.fromLayer === movement.fromLayer && candidate.toLayer === movement.toLayer,
    )
    const counter = rule ? `${movement.returnCount} de ${rule.returnLimit}` : `devolução ${movement.returnCount}`
    return `devolveu ${item ? `${item} ` : ''}a ${target} · ${counter}`
  }
  if (movement.kind === 'implementar') {
    return `delegou ${item ? `${item} ` : ''}a ${target}`
  }
  return `${REQUEST_VERBS[movement.kind]}${item ? ` de ${item}` : ''} a ${target}`
}

export function formatMovementTime(createdAt: number): string {
  return new Date(createdAt).toLocaleTimeString(LOCALE, MOVEMENT_TIME_OPTIONS)
}

export const WHEN_EVENT_LABELS: Record<WhenEvent, string> = {
  plano_aprovado: 'Plano aprovado',
  tarefa_concluida: 'Tarefa concluída',
  revisao_reprovou: 'Revisão reprovou',
  testes_falharam: 'Testes falharam',
  revisao_aprovou: 'Revisão aprovou',
  pedido_de_ajuda: 'Agente pede ajuda',
}

export const THEN_CHOICE_LABELS: Record<ThenChoice, string> = {
  delegar: 'Delegar',
  pedir_revisao: 'Pedir revisão',
  pesquisar_antes: 'Pesquisar antes',
  devolver: 'Devolver',
  chamar_voce: 'Chamar você',
}

export const EXEC_MODE_LABELS: Record<ExecMode, string> = {
  sequencia: 'Uma por vez',
  paralelo: 'Em paralelo',
}

export const STEP_MODE_LABELS: Record<StepMode, string> = {
  lotes: 'Por lotes',
  livre: 'Livre',
}

export const HUMAN_GATE_LABELS: Record<HumanGate, string> = {
  lote: 'A cada lote',
  fim: 'Só no fim',
  nunca: 'Nunca',
}

export const WORKTREE_LABELS: Record<WorktreePolicy, string> = {
  sempre: 'Sempre',
  paralelo: 'Com paralelo',
  nunca: 'Nunca',
}

export const VIEW_COPY = {
  flowTitle: 'Fluxo entre camadas',
  addArrow: '+ Seta',
  ruleTitle: 'Regra da seta',
  newArrowTitle: 'Nova seta',
  from: 'De',
  to: 'Para',
  when: 'Quando',
  then: 'Então',
  exec: 'Execução',
  returns: 'Devoluções antes de chamar você',
  createArrow: 'Criar seta',
  cancel: 'Cancelar',
  removeArrow: 'Remover seta',
  removeArrowConfirm: 'Remover esta seta?',
  removeArrowAction: 'Remover',
  close: 'Fechar',
  leaderTitle: 'conduz',
  leaderPick: 'Quem conduz',
  steps: 'Etapas',
  humanGate: 'Parar para você',
  parallel: 'Agentes ao mesmo tempo',
  stepsOrder: 'Ordem das etapas',
  you: 'Você',
  youApprove: 'Você aprova',
  approvesMerge: 'aprova o merge',
  autoMerge: 'Merge automático',
  project: 'Projeto',
  pairTurns: 'Turnos por par',
  taskTurns: 'Turnos por tarefa',
  worktree: 'Worktree',
  enforcement: 'Aplicar as regras do mapa',
  enforcementWarning: 'Sem setas, toda delegação é recusada.',
  layerTitle: 'Camada de orquestração',
  saveLayer: 'Salvar camada',
  deleteLayer: 'Excluir camada',
  deleteLayerConfirm: 'Excluir esta camada? As setas ligadas a ela também saem.',
  moveLeft: '← Mover',
  moveRight: 'Mover →',
  instructions: 'Instruções',
  instances: 'Instâncias',
  saveAgent: 'Salvar agente',
  noPlacementPossible: 'Crie uma camada e um time antes de posicionar agentes.',
  noLayersTitle: 'Nenhuma camada ainda.',
  createLayer: 'Criar camada',
  unplaced: 'Sem posição',
} as const
