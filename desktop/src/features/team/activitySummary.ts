import type { ToolActivity, ToolCategory, ToolChange, ToolDetail, ToolStatus } from './events'
import { fileIconOf, type FileIconKind } from './fileIcon'

type ReadDetail = Extract<ToolDetail, { kind: 'read' }>
type FoundDetail = Extract<ToolDetail, { kind: 'found' }>
type TaskUpdatedDetail = Extract<ToolDetail, { kind: 'updated' }>
type StartedDetail = Extract<ToolDetail, { kind: 'started' }>

const MS_PER_SECOND = 1000
const SECONDS_PER_MINUTE = 60
const MINUTES_PER_HOUR = 60
const MIN_THINKING_MS = MS_PER_SECOND
const PATH_SEPARATOR = /[\\/]/
const MINUS = '−'
const VOKIA_TOOL_PREFIX = 'mcp__vokia__'
const FOUND_VERB = 'Encontrou'
const TASKS_CREATED_VERB = 'Tarefas criadas'
// Só "completed" foi capturado; outros status caem no verbo genérico até haver captura.
const TASK_UPDATE_VERBS: Partial<Record<string, string>> = { completed: 'Concluiu tarefa' }
const TASK_UPDATE_FALLBACK_VERB = 'Atualizou tarefa'
const STARTED_VERB = 'Iniciou tarefa'
const TASK_STARTED_VERB = 'Task iniciada'
const TASK_STARTED_ID = 'task-started'
export type CardTitleOf = (cardId: string) => string | undefined

type ClauseGroup = 'read' | 'search' | 'list' | 'command' | 'edit' | 'web' | 'mcp' | 'agent' | 'tool'

const GROUP_OF: Record<ToolCategory, ClauseGroup> = {
  read: 'read',
  search: 'search',
  list: 'list',
  command: 'command',
  edit: 'edit',
  web: 'web',
  mcp: 'mcp',
  agent: 'agent',
  plan: 'tool',
  other: 'tool',
}

const CLAUSE: Record<ClauseGroup, (count: number) => string> = {
  read: (count) => `leu ${count} ${count === 1 ? 'arquivo' : 'arquivos'}`,
  search: (count) => `buscou ${count} ${count === 1 ? 'vez' : 'vezes'}`,
  list: (count) => `listou ${count} ${count === 1 ? 'diretório' : 'diretórios'}`,
  command: (count) => `executou ${count} ${count === 1 ? 'comando' : 'comandos'}`,
  edit: (count) => `editou ${count} ${count === 1 ? 'arquivo' : 'arquivos'}`,
  web: (count) => `buscou na web ${count} ${count === 1 ? 'vez' : 'vezes'}`,
  mcp: (count) => `usou ${count} ${count === 1 ? 'integração' : 'integrações'}`,
  agent: (count) => `executou ${count} ${count === 1 ? 'agente' : 'agentes'}`,
  tool: (count) => `usou ${count} ${count === 1 ? 'ferramenta' : 'ferramentas'}`,
}

interface RowVerbs {
  running: string
  done: string
  failed: string
}

const ROW_VERBS: Record<ToolCategory, RowVerbs> = {
  read: { running: 'Lendo', done: 'Leu', failed: 'Tentou ler' },
  edit: { running: 'Editando', done: 'Editou', failed: 'Tentou editar' },
  command: { running: 'Executando', done: 'Executou', failed: 'Tentou executar' },
  search: { running: 'Buscando', done: 'Buscou', failed: 'Tentou buscar' },
  list: { running: 'Listando', done: 'Listou', failed: 'Tentou listar' },
  web: { running: 'Buscando na web', done: 'Buscou na web', failed: 'Tentou buscar na web' },
  mcp: { running: 'Usando', done: 'Usou', failed: 'Tentou usar' },
  agent: { running: 'Executando agente', done: 'Executou agente', failed: 'Tentou executar agente' },
  plan: { running: 'Atualizando o plano', done: 'Atualizou o plano', failed: 'Tentou atualizar o plano' },
  other: { running: 'Usando', done: 'Usou', failed: 'Tentou usar' },
}

export type RowIconKind = FileIconKind | 'folder' | 'search' | 'edit' | 'command' | 'web' | 'other'

const ROW_ICON: Record<ToolCategory, RowIconKind> = {
  read: 'document',
  edit: 'edit',
  command: 'command',
  search: 'search',
  list: 'folder',
  web: 'web',
  mcp: 'other',
  agent: 'other',
  plan: 'other',
  other: 'other',
}

const FILE_CATEGORIES: readonly ToolCategory[] = ['read', 'edit']
// Padrão de busca ou caminho de listagem: o alvo inteiro vira o chip, sem cortar pelo separador.
const PATTERN_CATEGORIES: readonly ToolCategory[] = ['search', 'list']
// Sem alvo, só o nome diz qual ferramenta rodou; as demais categorias já têm um verbo próprio.
const NAMED_CATEGORIES: readonly ToolCategory[] = ['mcp', 'other']

export interface ActivityChip {
  label: string
  title: string
  icon: FileIconKind
}

export interface ActivityRowModel {
  id: string
  icon: RowIconKind
  verb: string
  chip?: ActivityChip
  text?: string
  meta?: string
  items?: string[]
  preview?: ToolChange
  failed: boolean
}

function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1)
}

// Contagem por grupo na ordem do primeiro uso, não por ranking.
export function activityClauses(tools: readonly ToolActivity[]): string[] {
  const counts = new Map<ClauseGroup, number>()
  for (const tool of tools) {
    const group = GROUP_OF[tool.category]
    counts.set(group, (counts.get(group) ?? 0) + 1)
  }
  return [...counts].map(([group, count]) => CLAUSE[group](count))
}

function joinClauses(clauses: readonly string[]): string {
  if (clauses.length <= 1) return clauses.join('')
  return `${clauses.slice(0, -1).join(', ')} e ${clauses[clauses.length - 1]}`
}

export function activitySentence(tools: readonly ToolActivity[]): string {
  const [only] = tools
  if (tools.length === 1 && only.category === 'command' && only.status !== 'running' && only.target !== '') {
    return only.target
  }
  return capitalize(joinClauses(activityClauses(tools)))
}

export function formatClock(ms: number): string {
  const totalSeconds = Math.floor(ms / MS_PER_SECOND)
  const seconds = String(totalSeconds % SECONDS_PER_MINUTE).padStart(2, '0')
  const totalMinutes = Math.floor(totalSeconds / SECONDS_PER_MINUTE)
  const minutes = totalMinutes % MINUTES_PER_HOUR
  const hours = Math.floor(totalMinutes / MINUTES_PER_HOUR)
  if (hours === 0) return `${minutes}:${seconds}`
  return `${hours}:${String(minutes).padStart(2, '0')}:${seconds}`
}

export function thinkingLabel(thinkingMs: number): string {
  return `Pensou por ${formatClock(Math.max(thinkingMs, MIN_THINKING_MS))}`
}

function verbFor(category: ToolCategory, status: ToolStatus): string {
  const verbs = ROW_VERBS[category]
  if (status === 'failed') return verbs.failed
  return status === 'running' ? verbs.running : verbs.done
}

function chipOf(target: string): ActivityChip {
  const label = target.split(PATH_SEPARATOR).pop() || target
  return { label, title: target, icon: fileIconOf(label) }
}

function patternChip(target: string): ActivityChip {
  return { label: target, title: target, icon: fileIconOf(target) }
}

function shownName(tool: ToolActivity): string {
  return tool.name.startsWith(VOKIA_TOOL_PREFIX) ? tool.name.slice(VOKIA_TOOL_PREFIX.length) : tool.name
}

function countLabel(count: number): string {
  return `${count} ${count === 1 ? 'arquivo' : 'arquivos'}`
}

function rangeLabel(from: number, to: number): string {
  return from === to ? `(L${from})` : `(L${from}-${to})`
}

function readRow(tool: ToolActivity, detail: ReadDetail, icon: RowIconKind): ActivityRowModel {
  return {
    id: tool.callId,
    icon,
    verb: verbFor(tool.category, tool.status),
    chip: chipOf(tool.target),
    meta: rangeLabel(detail.from, detail.to),
    failed: tool.status === 'failed',
  }
}

function foundRow(tool: ToolActivity, detail: FoundDetail, icon: RowIconKind): ActivityRowModel {
  return {
    id: tool.callId,
    icon,
    verb: tool.status === 'ok' ? FOUND_VERB : verbFor(tool.category, tool.status),
    chip: patternChip(tool.target),
    meta: `· ${countLabel(detail.count)}`,
    items: detail.names.length > 0 ? detail.names : undefined,
    failed: tool.status === 'failed',
  }
}

function defaultRow(tool: ToolActivity, icon: RowIconKind, failed: boolean): ActivityRowModel {
  const target = tool.target === '' && NAMED_CATEGORIES.includes(tool.category) ? shownName(tool) : tool.target
  const showChanges = tool.status === 'ok' && tool.added !== undefined && tool.removed !== undefined
  const meta = showChanges ? `+${tool.added} ${MINUS}${tool.removed}` : undefined
  const verb = verbFor(tool.category, tool.status)
  if (FILE_CATEGORIES.includes(tool.category) && target !== '') {
    return { id: tool.callId, icon, verb, chip: chipOf(target), meta, preview: tool.change, failed }
  }
  if (PATTERN_CATEGORIES.includes(tool.category) && target !== '') {
    return { id: tool.callId, icon, verb, chip: patternChip(target), meta, failed }
  }
  return { id: tool.callId, icon, verb, text: target === '' ? undefined : target, meta, failed }
}

// Uma linha só para todas as tarefas criadas, na posição da primeira; o nome de cada uma vai na lista.
function taskCreatedRow(created: readonly ToolActivity[]): ActivityRowModel {
  const [first] = created
  return {
    id: first.callId,
    icon: ROW_ICON[first.category],
    verb: TASKS_CREATED_VERB,
    meta: `· ${created.length} ${created.length === 1 ? 'tarefa' : 'tarefas'}`,
    items: created.map((tool) => tool.target),
    failed: false,
  }
}

function taskUpdatedRow(tool: ToolActivity, detail: TaskUpdatedDetail, created: readonly ToolActivity[]): ActivityRowModel {
  const verb = TASK_UPDATE_VERBS[detail.status] ?? TASK_UPDATE_FALLBACK_VERB
  const icon = ROW_ICON[tool.category]
  const failed = tool.status === 'failed'
  const index = created.findIndex((task) => task.detail?.kind === 'created' && task.detail.id === detail.taskId)
  if (index === -1) {
    return { id: tool.callId, icon, verb, text: `#${detail.taskId}`, failed }
  }
  return {
    id: tool.callId,
    icon,
    verb,
    meta: `· ${index + 1}/${created.length}`,
    text: created[index].target,
    failed,
  }
}

// Sem título do card no backlog, a linha não aparece: o id sozinho não diz qual tarefa começou.
function startedRow(tool: ToolActivity, detail: StartedDetail, titleOf: CardTitleOf): ActivityRowModel[] {
  const title = titleOf(detail.cardId)
  if (tool.status !== 'ok' || title === undefined) return []
  return [{ id: tool.callId, icon: ROW_ICON[tool.category], verb: STARTED_VERB, text: title, failed: false }]
}

// O prompt inteiro fica na lista expansível; a linha mostra só o título da task.
export function taskStartedRow(title: string, prompt: string): ActivityRowModel {
  return {
    id: TASK_STARTED_ID,
    icon: 'command',
    verb: TASK_STARTED_VERB,
    chip: { label: title, title, icon: 'generic' },
    items: prompt.split('\n').filter((line) => line.trim() !== ''),
    failed: false,
  }
}

export function activityRows(tools: readonly ToolActivity[], titleOf: CardTitleOf): ActivityRowModel[] {
  // Criação conta desde a entrada da ferramenta: a linha nasce uma vez e só cresce com o contador.
  const created = tools.filter((tool) => tool.detail?.kind === 'created')
  return tools.flatMap((tool) => {
    const icon = ROW_ICON[tool.category]
    if (tool.detail?.kind === 'created') return tool === created[0] ? [taskCreatedRow(created)] : []
    if (tool.detail?.kind === 'started') return startedRow(tool, tool.detail, titleOf)
    if (tool.detail?.kind === 'updated') return [taskUpdatedRow(tool, tool.detail, created)]
    if (tool.detail?.kind === 'read') return [readRow(tool, tool.detail, icon)]
    if (tool.detail?.kind === 'found') return [foundRow(tool, tool.detail, icon)]
    return [defaultRow(tool, icon, tool.status === 'failed')]
  })
}
