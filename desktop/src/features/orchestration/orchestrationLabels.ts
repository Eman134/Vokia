import type { Agent } from '../team/events'
import {
  VOCE_NODE,
  type LayerKind,
  type OrchAction,
  type OrchMovement,
  type OrchRule,
  type RequestKind,
  type WhenEvent,
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
