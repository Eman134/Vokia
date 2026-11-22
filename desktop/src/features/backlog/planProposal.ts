import type { BacklogItem, ScriptBlock } from './backlogTypes'
import type { PlanDepth } from './useItemPlan'

export const PLAN_READY_MARKER = '#plano-pronto'
const ESTIMATE_MARKER = '#estimativa'
const LOT_MARKER = '#lote'
const SCRIPT_MARKER = '#roteiro'
const RISK_MARKER = '#risco'
const TAGS_MARKER = '#etiquetas'
const LOT_FIELD_SEPARATOR = '|'
const SCRIPT_ENTRY_SEPARATOR = '~'
const SCRIPT_KEY_SEPARATOR = ':'
const FILE_SEPARATOR = ','
const TAG_SEPARATOR = ','
const TASK_SEPARATOR = ';'
const TASKS_JOINER = `${TASK_SEPARATOR} `
const LIST_JOINER = ', '
const LINE_SEPARATOR = '\n'
const PARAGRAPH_SEPARATOR = '\n\n'
const LOT_NUMBER_PATTERN = /^\d+$/
const LINE_BREAKS = /\s*\n\s*/g
const SCRIPT_KEY = {
  who: 'quem',
  model: 'modelo',
  context: 'ctx',
  files: 'arq',
  step: 'passo',
  done: 'pronto',
  limit: 'nao',
} as const

const RESEARCH_INSTRUCTION =
  'Pesquise o código relevante da pasta do projeto com as ferramentas de leitura, entenda o que deve ser feito e decomponha em lotes pequenos e verificáveis.'
const QUICK_INSTRUCTION = 'Seja rápido: um lote e sem pesquisa extensa.'
const FULL_INSTRUCTION = 'Faça uma pesquisa completa antes de propor.'
const QUESTION_INSTRUCTION =
  'Se faltar informação, pergunte antes de propor terminando a resposta com uma linha: #decisao pergunta|<título>|<detalhe>|<opção1>;<opção2>. Quando o usuário responder, continue a pesquisa.'
const USER_NOTE_PREFIX = 'Instruções do usuário (têm prioridade sobre o resto):'
const FINAL_INSTRUCTION =
  'Quando terminar, responda em português com a primeira linha #plano-pronto e depois, um marcador por linha: #estimativa ~<tempo>, uma linha #lote <n>|<título>|<meta>|<tarefa>; <tarefa> por lote, se quiser uma linha #roteiro <n>|quem:<agente>~modelo:<modelo>~ctx:<contexto>~arq:<arquivo>, <arquivo>~passo:<passo>~pronto:<critério>~nao:<limite> por lote (passo, pronto e nao podem se repetir), #risco <frase curta>, #etiquetas <etiqueta>, <etiqueta> (livres, em minúsculas) e, se quiser, um resumo. Mesmo depois das respostas do usuário, termine sempre com #plano-pronto.'
const IMPORT_INSTRUCTION =
  'Converta o plano abaixo para o formato do Vokia sem pesquisa extensa: mantenha as etapas e os critérios do original, cada etapa vira um #lote e os critérios dela entram como pronto no #roteiro do lote. Escreva um #roteiro para o lote sempre que o original detalhar quem faz, arquivos, passos ou limites da etapa.'

export interface LotScript {
  who?: string
  model?: string
  block: ScriptBlock
}

export interface ProposalLot {
  n: number
  title: string
  meta: string
  tasks: string
  script?: LotScript
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

function parseLotScript(rest: string): { n: number; script: LotScript } | undefined {
  const pipeIndex = rest.indexOf(LOT_FIELD_SEPARATOR)
  if (pipeIndex === -1) return undefined
  const lotField = rest.slice(0, pipeIndex).trim()
  if (!LOT_NUMBER_PATTERN.test(lotField)) return undefined
  const n = Number(lotField)
  const body = rest.slice(pipeIndex + 1).trim()
  if (n < 1 || body === '') return undefined

  let who: string | undefined
  let model: string | undefined
  const block: ScriptBlock = { context: '', files: [], steps: [], done: [], limits: [] }

  for (const rawEntry of body.split(SCRIPT_ENTRY_SEPARATOR)) {
    const entry = rawEntry.trim()
    if (entry === '') continue
    const colonIndex = entry.indexOf(SCRIPT_KEY_SEPARATOR)
    if (colonIndex === -1) return undefined
    const key = entry.slice(0, colonIndex).trim().toLowerCase()
    const value = entry.slice(colonIndex + 1).trim()
    switch (key) {
      case SCRIPT_KEY.who:
        if (value !== '') who = value
        break
      case SCRIPT_KEY.model:
        if (value !== '') model = value
        break
      case SCRIPT_KEY.context:
        block.context = value
        break
      case SCRIPT_KEY.files:
        block.files = value
          .split(FILE_SEPARATOR)
          .map((file) => file.trim())
          .filter((file) => file !== '')
        break
      case SCRIPT_KEY.step:
        if (value !== '') block.steps.push(value)
        break
      case SCRIPT_KEY.done:
        if (value !== '') block.done.push(value)
        break
      case SCRIPT_KEY.limit:
        if (value !== '') block.limits.push(value)
        break
      default:
        return undefined
    }
  }

  return { n, script: { ...(who ? { who } : {}), ...(model ? { model } : {}), block } }
}

export function parseProposal(text: string): ParsedProposal {
  const parsed: ParsedProposal = { estimate: '', lots: [], risk: '', tags: [], summary: '' }
  const summary: string[] = []
  const scriptsByLot = new Map<number, LotScript>()
  for (const raw of text.split(LINE_SEPARATOR)) {
    const line = raw.trim()
    if (line === PLAN_READY_MARKER) continue
    if (line.startsWith(ESTIMATE_MARKER)) {
      parsed.estimate = line.slice(ESTIMATE_MARKER.length).trim()
    } else if (line.startsWith(LOT_MARKER)) {
      const lot = parseLot(line.slice(LOT_MARKER.length), parsed.lots.length + 1)
      if (lot) parsed.lots.push(lot)
    } else if (line.startsWith(SCRIPT_MARKER)) {
      const parsedScript = parseLotScript(line.slice(SCRIPT_MARKER.length))
      if (parsedScript) scriptsByLot.set(parsedScript.n, parsedScript.script)
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
  for (const lot of parsed.lots) {
    const script = scriptsByLot.get(lot.n)
    if (script) lot.script = script
  }
  parsed.summary = summary.join(LINE_SEPARATOR).trim()
  return parsed
}

function userNoteParagraphs(item: BacklogItem): string[] {
  const note = item.planNote?.trim()
  return note ? [`${USER_NOTE_PREFIX}\n${note}`] : []
}

export function buildPlanPrompt(item: BacklogItem, depth: PlanDepth): string {
  const parts = [`Triagem do item "${item.title}".`]
  if (item.description) parts.push(`Descrição: ${item.description}`)
  if (item.criteria?.length) parts.push(`Critérios:\n${item.criteria.map((criterion) => `- ${criterion.text}`).join(LINE_SEPARATOR)}`)
  parts.push(RESEARCH_INSTRUCTION, depth === 'rapido' ? QUICK_INSTRUCTION : FULL_INSTRUCTION, QUESTION_INSTRUCTION)
  parts.push(...userNoteParagraphs(item), FINAL_INSTRUCTION)
  return parts.join(PARAGRAPH_SEPARATOR)
}

export function buildAdjustPrompt(item: BacklogItem, proposal: string, instruction: string): string {
  return [
    `Ajuste o plano do item "${item.title}" conforme o pedido do usuário: ${instruction}`,
    `Plano atual:\n${proposal}`,
    ...userNoteParagraphs(item),
    FINAL_INSTRUCTION,
  ].join(PARAGRAPH_SEPARATOR)
}

export function buildImportPrompt(title: string, text: string): string {
  return [`Plano "${title}".`, IMPORT_INSTRUCTION, `Plano original:\n${text}`, FINAL_INSTRUCTION].join(PARAGRAPH_SEPARATOR)
}

// Os separadores do protocolo dentro de um campo quebrariam a linha ao ser lida de volta por parseProposal.
function protocolField(value: string, separators: readonly string[]): string {
  return separators.reduce((text, separator) => text.split(separator).join(' '), value.replace(LINE_BREAKS, ' ')).trim()
}

function lotLine(lot: ProposalLot, n: number): string {
  const fields = [lot.title, lot.meta, lot.tasks].map((field) => protocolField(field, [LOT_FIELD_SEPARATOR]))
  return `${LOT_MARKER} ${[String(n), ...fields].join(LOT_FIELD_SEPARATOR)}`
}

function scriptLine(script: LotScript, n: number): string | undefined {
  const entry = (key: string, value: string): string[] => {
    const clean = protocolField(value, [SCRIPT_ENTRY_SEPARATOR])
    return clean === '' ? [] : [`${key}${SCRIPT_KEY_SEPARATOR}${clean}`]
  }
  const files = script.block.files.map((file) => protocolField(file, [SCRIPT_ENTRY_SEPARATOR, FILE_SEPARATOR]))
  const entries = [
    ...entry(SCRIPT_KEY.who, script.who ?? ''),
    ...entry(SCRIPT_KEY.model, script.model ?? ''),
    ...entry(SCRIPT_KEY.context, script.block.context),
    ...entry(SCRIPT_KEY.files, files.filter((file) => file !== '').join(LIST_JOINER)),
    ...script.block.steps.flatMap((step) => entry(SCRIPT_KEY.step, step)),
    ...script.block.done.flatMap((done) => entry(SCRIPT_KEY.done, done)),
    ...script.block.limits.flatMap((limit) => entry(SCRIPT_KEY.limit, limit)),
  ]
  return entries.length === 0 ? undefined : `${SCRIPT_MARKER} ${n}${LOT_FIELD_SEPARATOR}${entries.join(SCRIPT_ENTRY_SEPARATOR)}`
}

// Inverso de parseProposal: os lotes são renumerados pela ordem, como o parser já faz ao ler.
export function serializeProposal(parsed: ParsedProposal): string {
  const lines = [PLAN_READY_MARKER]
  const estimate = protocolField(parsed.estimate, [])
  if (estimate !== '') lines.push(`${ESTIMATE_MARKER} ${estimate}`)
  parsed.lots.forEach((lot, index) => {
    const n = index + 1
    lines.push(lotLine(lot, n))
    const script = lot.script ? scriptLine(lot.script, n) : undefined
    if (script) lines.push(script)
  })
  const risk = protocolField(parsed.risk, [])
  if (risk !== '') lines.push(`${RISK_MARKER} ${risk}`)
  const tags = parsed.tags.map((tag) => protocolField(tag, [TAG_SEPARATOR])).filter((tag) => tag !== '')
  if (tags.length > 0) lines.push(`${TAGS_MARKER} ${tags.join(LIST_JOINER)}`)
  const summary = parsed.summary.trim()
  if (summary !== '') lines.push('', summary)
  return lines.join(LINE_SEPARATOR)
}

export function splitTasks(tasks: string): string[] {
  return tasks
    .split(TASK_SEPARATOR)
    .map((task) => task.trim())
    .filter((task) => task !== '')
}

export function joinTasks(tasks: readonly string[]): string {
  return tasks
    .map((task) => task.trim())
    .filter((task) => task !== '')
    .join(TASKS_JOINER)
}
