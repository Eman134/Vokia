import type { BacklogItem } from './backlogTypes'
import type { PlanDepth } from './useItemPlan'

export const PLAN_READY_MARKER = '#plano-pronto'
const ESTIMATE_MARKER = '#estimativa'
const LOT_MARKER = '#lote'
const RISK_MARKER = '#risco'
const TAGS_MARKER = '#etiquetas'
const LOT_FIELD_SEPARATOR = '|'
const TAG_SEPARATOR = ','
const LINE_SEPARATOR = '\n'
const PARAGRAPH_SEPARATOR = '\n\n'
const LOT_NUMBER_PATTERN = /^\d+$/

const RESEARCH_INSTRUCTION =
  'Pesquise o código relevante da pasta do projeto com as ferramentas de leitura, entenda o que deve ser feito e decomponha em lotes pequenos e verificáveis.'
const QUICK_INSTRUCTION = 'Seja rápido: um lote e sem pesquisa extensa.'
const FULL_INSTRUCTION = 'Faça uma pesquisa completa antes de propor.'
const QUESTION_INSTRUCTION =
  'Se faltar informação, pergunte antes de propor terminando a resposta com uma linha: #decisao pergunta|<título>|<detalhe>|<opção1>;<opção2>. Quando o usuário responder, continue a pesquisa.'
const FINAL_INSTRUCTION =
  'Quando terminar, responda em português com a primeira linha #plano-pronto e depois, um marcador por linha: #estimativa ~<tempo>, uma linha #lote <n>|<título>|<meta>|<tarefa>; <tarefa> por lote, #risco <frase curta>, #etiquetas <etiqueta>, <etiqueta> (livres, em minúsculas) e, se quiser, um resumo. Mesmo depois das respostas do usuário, termine sempre com #plano-pronto.'

export interface ProposalLot {
  n: number
  title: string
  meta: string
  tasks: string
}

export interface ParsedProposal {
  estimate: string
  lots: ProposalLot[]
  risk: string
  tags: string[]
  summary: string
}

export function isFinalReply(text: string): boolean {
  return text.split(LINE_SEPARATOR).some((line) => line.trim() === PLAN_READY_MARKER)
}

function parseLot(rest: string, n: number): ProposalLot | undefined {
  const fields = rest.split(LOT_FIELD_SEPARATOR).map((field) => field.trim())
  if (fields[0] !== undefined && LOT_NUMBER_PATTERN.test(fields[0])) fields.shift()
  const [title = '', meta = '', tasks = ''] = fields
  return title === '' ? undefined : { n, title, meta, tasks }
}

export function parseProposal(text: string): ParsedProposal {
  const parsed: ParsedProposal = { estimate: '', lots: [], risk: '', tags: [], summary: '' }
  const summary: string[] = []
  for (const raw of text.split(LINE_SEPARATOR)) {
    const line = raw.trim()
    if (line === PLAN_READY_MARKER) continue
    if (line.startsWith(ESTIMATE_MARKER)) {
      parsed.estimate = line.slice(ESTIMATE_MARKER.length).trim()
    } else if (line.startsWith(LOT_MARKER)) {
      const lot = parseLot(line.slice(LOT_MARKER.length), parsed.lots.length + 1)
      if (lot) parsed.lots.push(lot)
    } else if (line.startsWith(RISK_MARKER)) {
      parsed.risk = line.slice(RISK_MARKER.length).trim()
    } else if (line.startsWith(TAGS_MARKER)) {
      parsed.tags = line
        .slice(TAGS_MARKER.length)
        .split(TAG_SEPARATOR)
        .map((tag) => tag.trim())
        .filter((tag) => tag !== '')
    } else {
      summary.push(raw)
    }
  }
  parsed.summary = summary.join(LINE_SEPARATOR).trim()
  return parsed
}

export function buildPlanPrompt(item: BacklogItem, depth: PlanDepth): string {
  const parts = [`Triagem do item "${item.title}".`]
  if (item.description) parts.push(`Descrição: ${item.description}`)
  if (item.criteria?.length) parts.push(`Critérios:\n${item.criteria.map((criterion) => `- ${criterion.text}`).join(LINE_SEPARATOR)}`)
  parts.push(RESEARCH_INSTRUCTION, depth === 'rapido' ? QUICK_INSTRUCTION : FULL_INSTRUCTION, QUESTION_INSTRUCTION, FINAL_INSTRUCTION)
  return parts.join(PARAGRAPH_SEPARATOR)
}

export function buildAdjustPrompt(item: BacklogItem, proposal: string, instruction: string): string {
  return [
    `Ajuste o plano do item "${item.title}" conforme o pedido do usuário: ${instruction}`,
    `Plano atual:\n${proposal}`,
    FINAL_INSTRUCTION,
  ].join(PARAGRAPH_SEPARATOR)
}
