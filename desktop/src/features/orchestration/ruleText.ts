import { WHEN_EVENT_PHRASES, layerName } from './orchestrationLabels'
import {
  THEN_CHOICE_VALUES,
  VOCE_NODE,
  type ExecMode,
  type LayerKind,
  type OrchRule,
  type RequestKind,
  type ThenChoice,
} from './orchestrationTypes'

const RETURN_KIND_BY_LAYER: Record<LayerKind, RequestKind> = {
  planejar: 'planejar',
  implementar: 'implementar',
  revisar: 'revisar',
  pesquisar: 'pesquisar',
  livre: 'implementar',
}

const THEN_VERBS: Record<ThenChoice, string> = {
  delegar: 'delega o trabalho para',
  pedir_revisao: 'pede revisão a',
  pesquisar_antes: 'pede pesquisa a',
  devolver: 'devolve para',
  chamar_voce: 'chama você',
}

const THEN_ARROW_LABELS: Record<ThenChoice, string> = {
  delegar: 'delega',
  pedir_revisao: 'pede revisão',
  pesquisar_antes: 'pede pesquisa',
  devolver: 'devolve',
  chamar_voce: 'chama você',
}

const EXEC_PHRASES: Record<ExecMode, string> = {
  sequencia: 'uma por vez',
  paralelo: 'em paralelo',
}

const PARALLEL_SUFFIX = ' · em paralelo'
const RETURN_LIMIT_UNIT_SINGULAR = 'vez'
const RETURN_LIMIT_UNIT_PLURAL = 'vezes'
const ARROW_RETURN_UNIT = 'x'

type ActionAndKind = Pick<OrchRule, 'action' | 'kind'>

export function thenChoiceOf(rule: ActionAndKind): ThenChoice {
  if (rule.action === 'escalar') return 'chamar_voce'
  if (rule.action === 'devolver') return 'devolver'
  if (rule.kind === 'revisar') return 'pedir_revisao'
  if (rule.kind === 'pesquisar') return 'pesquisar_antes'
  return 'delegar'
}

// Destino Você só admite "chamar você"; uma camada admite as outras opções e também "chamar você", que leva o destino para Você.
export function thenChoicesFor(toLayer: string): ThenChoice[] {
  if (toLayer === VOCE_NODE) return ['chamar_voce']
  return [...THEN_CHOICE_VALUES]
}

export function actionAndKindOf(choice: ThenChoice, targetKind?: LayerKind): ActionAndKind {
  switch (choice) {
    case 'chamar_voce':
      return { action: 'escalar', kind: 'revisar' }
    case 'devolver':
      if (targetKind === undefined) throw new Error('devolver precisa da camada de destino')
      return { action: 'devolver', kind: RETURN_KIND_BY_LAYER[targetKind] }
    case 'pedir_revisao':
      return { action: 'delegar', kind: 'revisar' }
    case 'pesquisar_antes':
      return { action: 'delegar', kind: 'pesquisar' }
    case 'delegar':
      return { action: 'delegar', kind: 'implementar' }
  }
}

export function arrowLabel(rule: OrchRule): string {
  const choice = thenChoiceOf(rule)
  const base =
    choice === 'devolver'
      ? `${THEN_ARROW_LABELS.devolver} · até ${rule.returnLimit}${ARROW_RETURN_UNIT}`
      : THEN_ARROW_LABELS[choice]
  return rule.action === 'escalar' || rule.execMode === 'sequencia' ? base : `${base}${PARALLEL_SUFFIX}`
}

export function rulePhrase(rule: OrchRule, layers: readonly { id: string; name: string }[]): string {
  const origin = layerName(layers, rule.fromLayer)
  const when = WHEN_EVENT_PHRASES[rule.whenEvent]
  if (rule.action === 'escalar') return `Quando ${when}, ${origin} chama você.`
  const target = layerName(layers, rule.toLayer)
  const verb = THEN_VERBS[thenChoiceOf(rule)]
  const exec = EXEC_PHRASES[rule.execMode]
  const limit =
    rule.action === 'devolver'
      ? `, até ${rule.returnLimit} ${rule.returnLimit === 1 ? RETURN_LIMIT_UNIT_SINGULAR : RETURN_LIMIT_UNIT_PLURAL} antes de chamar você`
      : ''
  return `Quando ${when}, ${origin} ${verb} ${target}, ${exec}${limit}.`
}
