import { STATE_CHIP } from '../backlog/backlogLabels'
import type { BacklogItem, BacklogState, ScriptBlock } from '../backlog/backlogTypes'
import { breakdownOf, type BreakdownStep } from '../backlog/planBreakdown'
import type { ParsedProposal } from '../backlog/planProposal'
import type { ArtifactCriterion } from './artifactApi'
import { matchStages, stageChip } from './planProgress'

export const PLAN_BLOCK_LANGUAGE = 'vokia-plano'
const FENCE_CHAR = '`'
const MIN_FENCE_LENGTH = 4
const BACKTICK_RUNS = /`+/g
const FENCE_LINE = /^(`{3,})\s*(\S*)\s*$/
const TITLE_LINE = /^#\s+(.+?)\s*#*\s*$/
const FILE_EXTENSION = /\.[^./\\]+$/
const PLAN_FILE_PREFIX = 'plano-'
const PLAN_FILE_EXTENSION = '.md'
const SLUG_MAX_LENGTH = 60
const DIACRITICS = /[̀-ͯ]/g
const NON_SLUG_CHARS = /[^a-z0-9]+/g
const EDGE_DASHES = /^-+|-+$/g
// Caracteres que o Windows recusa em nome de arquivo.
const WINDOWS_INVALID_CHARS = /[<>:"/\\|?*\u0000-\u001f]/g
const LINE_SEPARATOR = '\n'
const LIST_JOINER = ', '
const PROPOSED_STATUS = 'proposta'

export interface PlanMarkdownInput {
  item: BacklogItem
  proposal: ParsedProposal
  rawText: string
  stages: readonly BacklogItem[]
  criteria: readonly ArtifactCriterion[]
  state: BacklogState
}

function bullets(values: readonly string[], indent = ''): string[] {
  return values.map((value) => `${indent}- ${value}`)
}

function scriptLines(script: ScriptBlock): string[] {
  const lines = ['', '#### Roteiro']
  if (script.context) lines.push(`- Contexto: ${script.context}`)
  if (script.files.length) lines.push(`- Arquivos: ${script.files.join(LIST_JOINER)}`)
  if (script.steps.length) lines.push('- Passos:', ...script.steps.map((step, index) => `  ${index + 1}. ${step}`))
  if (script.done.length) lines.push('- Pronto quando:', ...bullets(script.done, '  '))
  if (script.limits.length) lines.push('- Não fazer:', ...bullets(script.limits, '  '))
  return lines
}

function stepLines(step: BreakdownStep, stage: BacklogItem | undefined): string[] {
  const lines = ['', `### ${step.n}. ${step.title}`, '']
  const who = step.who ?? stage?.who.trim()
  if (who) lines.push(`- Quem: ${who}`)
  lines.push(`- Status: ${stage ? stageChip(stage).label : PROPOSED_STATUS}`)
  if (step.goal) lines.push(`- Meta: ${step.goal}`)
  if (step.tasks.length) lines.push('- Tarefas:', ...bullets(step.tasks, '  '))
  if (step.script) lines.push(...scriptLines(step.script))
  return lines
}

// A cerca precisa ser mais longa que qualquer sequência de crases do conteúdo, senão o bloco fecha no meio.
function fenceFor(content: string): string {
  const longest = Math.max(0, ...(content.match(BACKTICK_RUNS) ?? []).map((run) => run.length))
  return FENCE_CHAR.repeat(Math.max(MIN_FENCE_LENGTH, longest + 1))
}

export function planToMarkdown({ item, proposal, rawText, stages, criteria, state }: PlanMarkdownInput): string {
  // Sem cards no breakdownOf: ele casa etapa e card só pela posição; o casamento certo vem de matchStages.
  const breakdown = breakdownOf(item, proposal, [])
  const cards = matchStages(item.id, stages, proposal.lots.map((lot) => lot.title))
  const header = [`Card: ${item.id}`, `Estado: ${STATE_CHIP[state]}`]
  if (breakdown.estimate) header.push(`Estimativa: ${breakdown.estimate}`)

  const lines = [`# ${item.title}`, '', header.join(' · ')]
  if (proposal.summary) lines.push('', proposal.summary)
  if (breakdown.steps.length) {
    lines.push('', '## Etapas')
    breakdown.steps.forEach((step, index) => lines.push(...stepLines(step, cards[index])))
  }
  if (criteria.length) {
    lines.push('', '## Critérios', '', ...criteria.map((criterion) => `- [${criterion.done ? 'x' : ' '}] ${criterion.text}`))
  }
  if (proposal.risk) lines.push('', '## Riscos', '', `- ${proposal.risk}`)
  if (proposal.tags.length) lines.push('', `Etiquetas: ${proposal.tags.join(LIST_JOINER)}`)

  const content = rawText.trim()
  const fence = fenceFor(content)
  lines.push('', `${fence}${PLAN_BLOCK_LANGUAGE}`, content, fence, '')
  return lines.join(LINE_SEPARATOR)
}

export function extractPlanBlock(text: string): string | undefined {
  const lines = text.replace(/\r\n?/g, LINE_SEPARATOR).split(LINE_SEPARATOR)
  const start = lines.findIndex((line) => {
    const fence = FENCE_LINE.exec(line.trim())
    return fence !== null && fence[2] === PLAN_BLOCK_LANGUAGE
  })
  if (start === -1) return undefined
  const openLength = FENCE_LINE.exec(lines[start].trim())![1].length
  const body: string[] = []
  for (const line of lines.slice(start + 1)) {
    const fence = FENCE_LINE.exec(line.trim())
    if (fence && fence[2] === '' && fence[1].length >= openLength) {
      const block = body.join(LINE_SEPARATOR).trim()
      return block === '' ? undefined : block
    }
    body.push(line)
  }
  return undefined
}

export function titleFromText(text: string, fileName: string): string {
  for (const line of text.replace(/\r\n?/g, LINE_SEPARATOR).split(LINE_SEPARATOR)) {
    const title = TITLE_LINE.exec(line.trim())?.[1]
    if (title) return title
  }
  return fileName.replace(FILE_EXTENSION, '').trim()
}

function slugOf(text: string): string {
  return text
    .normalize('NFD')
    .replace(DIACRITICS, '')
    .toLowerCase()
    .replace(NON_SLUG_CHARS, '-')
    .slice(0, SLUG_MAX_LENGTH)
    .replace(EDGE_DASHES, '')
}

export function planFileName(item: BacklogItem): string {
  const card = item.id.replace(WINDOWS_INVALID_CHARS, '-')
  const slug = slugOf(item.title)
  return `${PLAN_FILE_PREFIX}${card}${slug ? `-${slug}` : ''}${PLAN_FILE_EXTENSION}`
}
