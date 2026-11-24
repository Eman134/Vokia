import { commandPhrase, friendlyCommand } from './appCommandLabel'
import type { CommandDomain, ToolActivity, ToolCategory, ToolChange, ToolDetail, ToolStatus } from './events'
import { fileIconOf, type FileIconKind } from './fileIcon'
import { isFinalReply, parseProposal } from '../backlog/planProposal'

type ReadDetail = Extract<ToolDetail, { kind: 'read' }>
type FoundDetail = Extract<ToolDetail, { kind: 'found' }>
type TaskUpdatedDetail = Extract<ToolDetail, { kind: 'updated' }>
type StartedDetail = Extract<ToolDetail, { kind: 'started' }>
type CommandDetail = Extract<ToolDetail, { kind: 'command' }>

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
const DELEGATION_COMMAND = /^maestri\s+ask\s+(?:"([^"]+)"|'([^']+)'|(\S+))\s*([\s\S]*)$/
const WRAPPING_QUOTES = /^(["'])([\s\S]*)\1$/
const LINE_BREAK = '\n'
const DELEGATION_VERB = 'Delegou a'
const DELEGATION_FAILED_VERB = 'Tentou delegar a'
const DELEGATION_WAITING = '· aguardando'
const DELEGATION_ANSWERED = '· respondeu'
const DELEGATION_DONE = '· concluída'
const THINKING_VERB = 'Pensou'
const THINKING_ROW_ID = 'thinking'
export type CardTitleOf = (cardId: string) => string | undefined
export type AgentNameOf = (id: string) => string | undefined

type ClauseGroup = 'read' | 'search' | 'list' | 'command' | 'edit' | 'web' | 'mcp' | 'app' | 'agent' | 'tool'

const GROUP_OF: Record<ToolCategory, ClauseGroup> = {
  read: 'read',
  search: 'search',
  list: 'list',
  command: 'command',
  edit: 'edit',
  web: 'web',
  mcp: 'mcp',
  app: 'app',
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
  app: (count) => `executou ${count} ${count === 1 ? 'ação' : 'ações'} no Vokia`,
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
  app: { running: 'Usando', done: 'Usou', failed: 'Tentou usar' },
  agent: { running: 'Executando agente', done: 'Executou agente', failed: 'Tentou executar agente' },
  plan: { running: 'Atualizando o plano', done: 'Atualizou o plano', failed: 'Tentou atualizar o plano' },
  other: { running: 'Usando', done: 'Usou', failed: 'Tentou usar' },
}

export type RowIconKind =
  | FileIconKind
  | 'folder'
  | 'search'
  | 'edit'
  | 'command'
  | 'delegate'
  | 'web'
  | 'thinking'
  | 'other'
  | 'role'
  | 'agent'
  | 'flow'
  | 'channel'
  | 'settings'
  | 'backlog'
  | 'project'
  | 'provider'
  | 'artifact'
  | 'attachment'
  | 'team'
  | 'check'
  | 'warning'

const ROW_ICON: Record<ToolCategory, RowIconKind> = {
  read: 'document',
  edit: 'edit',
  command: 'command',
  search: 'search',
  list: 'folder',
  web: 'web',
  mcp: 'other',
  app: 'other',
  agent: 'other',
  plan: 'other',
  other: 'other',
}

const DOMAIN_ICON: Record<CommandDomain, RowIconKind> = {
  team: 'team',
  projects: 'project',
  channels: 'channel',
  backlog: 'backlog',
  artifacts: 'artifact',
  attachments: 'attachment',
  agents: 'agent',
  roles: 'role',
  orchestration: 'flow',
  providers: 'provider',
  settings: 'settings',
}

const FILE_CATEGORIES: readonly ToolCategory[] = ['read', 'edit']
// Padrão de busca, caminho de listagem ou comando: o alvo inteiro vira o chip, sem cortar pelo separador.
const WHOLE_TARGET_CATEGORIES: readonly ToolCategory[] = ['search', 'list', 'command']
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
  // Comando cru sob a frase do agente, quando ele explicou o que o comando faz.
  command?: string
  failed: boolean
  // Ocorrências consecutivas idênticas fundidas nesta linha; ausente quando é uma só.
  count?: number
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
    return only.description ?? only.target
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

export function thinkingRow(thinkingMs: number): ActivityRowModel {
  const seconds = Math.floor(Math.max(thinkingMs, MIN_THINKING_MS) / MS_PER_SECOND)
  return { id: THINKING_ROW_ID, icon: 'thinking', verb: THINKING_VERB, text: `${seconds}s`, failed: false }
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

interface Delegation {
  agent: string
  prompt: string
}

function delegationOf(tool: ToolActivity): Delegation | undefined {
  if (tool.category !== 'command') return undefined
  const match = DELEGATION_COMMAND.exec(tool.target)
  if (!match) return undefined
  return { agent: match[1] ?? match[2] ?? match[3], prompt: match[4].trim().replace(WRAPPING_QUOTES, '$2') }
}

// O estado do destino não chega à atividade: sem resposta a linha diz só "aguardando", com ela "respondeu".
function delegationRow(tool: ToolActivity, delegation: Delegation): ActivityRowModel {
  const failed = tool.status === 'failed'
  const lines = tool.result?.split(LINE_BREAK).filter((line) => line.trim() !== '')
  const answered = tool.status === 'ok' && lines !== undefined && lines.length > 0
  const state = answered ? `${DELEGATION_ANSWERED} · ${lines[0]}` : tool.status === 'ok' ? DELEGATION_DONE : DELEGATION_WAITING
  return {
    id: tool.callId,
    icon: 'delegate',
    verb: failed ? DELEGATION_FAILED_VERB : DELEGATION_VERB,
    chip: { label: delegation.agent, title: delegation.prompt, icon: 'generic' },
    meta: failed ? undefined : state,
    items: answered ? lines : undefined,
    failed,
  }
}

// Domínio que o front ainda não conhece (catálogo mais novo) cai no ícone genérico.
function domainIcon(domain: string | undefined): RowIconKind {
  return domain !== undefined && Object.hasOwn(DOMAIN_ICON, domain) ? DOMAIN_ICON[domain as CommandDomain] : ROW_ICON.app
}

// O subject é o id do agente quando o comando é do domínio agents ou tem agent no nome; id fora do elenco fica como veio.
function subjectLabel(detail: CommandDetail, nameOf: AgentNameOf): string | undefined {
  if (detail.subject === undefined) return undefined
  const aboutAgent = detail.domain === 'agents' || detail.command.split('_').includes('agent')
  return aboutAgent ? (nameOf(detail.subject) ?? detail.subject) : detail.subject
}

function commandRow(tool: ToolActivity, detail: CommandDetail, nameOf: AgentNameOf): ActivityRowModel {
  return {
    id: tool.callId,
    icon: domainIcon(detail.domain),
    verb: commandPhrase(detail.command, tool.status),
    chip: { label: subjectLabel(detail, nameOf) ?? detail.command, title: friendlyCommand(detail.command), icon: 'generic' },
    failed: tool.status === 'failed',
  }
}

function defaultRow(tool: ToolActivity, icon: RowIconKind, failed: boolean): ActivityRowModel {
  if (tool.category === 'command' && tool.description !== undefined) {
    return { id: tool.callId, icon, verb: tool.description, command: tool.target, failed }
  }
  const target = tool.target === '' && NAMED_CATEGORIES.includes(tool.category) ? shownName(tool) : tool.target
  const showChanges = tool.status === 'ok' && tool.added !== undefined && tool.removed !== undefined
  const meta = showChanges ? `+${tool.added} ${MINUS}${tool.removed}` : undefined
  const verb = verbFor(tool.category, tool.status)
  if (FILE_CATEGORIES.includes(tool.category) && target !== '') {
    return { id: tool.callId, icon, verb, chip: chipOf(target), meta, preview: tool.change, failed }
  }
  if (WHOLE_TARGET_CATEGORIES.includes(tool.category) && target !== '') {
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

const TASK_CONCLUDED_VERB = 'Tarefa concluída'
const TASK_INTERRUPTED_VERB = 'Tarefa interrompida'

// Resultado da tarefa em uma linha; o detalhe (turnos, agentes, hora) fica no tooltip do objetivo.
export function taskResultRow(id: string, objective: string, concluded: boolean, detail: string): ActivityRowModel {
  return {
    id,
    icon: concluded ? 'check' : 'warning',
    verb: concluded ? TASK_CONCLUDED_VERB : TASK_INTERRUPTED_VERB,
    chip: { label: objective, title: `${objective} — ${detail}`, icon: 'generic' },
    failed: !concluded,
  }
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

const PROTOCOL_PREFIXES: ReadonlyArray<{ prefix: string; verb: string; quoted?: boolean }> = [
  { prefix: 'Triagem do item ', verb: 'Triagem' },
  { prefix: 'Ajuste o plano do item ', verb: 'Ajuste de plano' },
  { prefix: 'Plano "', verb: 'Planejamento', quoted: true },
]

function extractProtocolTitle(firstLine: string, prefix: string, quoted?: boolean): string {
  const rest = firstLine.slice(prefix.length).trim()
  if (quoted) {
    const endQuote = rest.indexOf('"')
    return endQuote >= 0 ? rest.slice(0, endQuote).trim() : rest
  }
  return rest.replace(WRAPPING_QUOTES, '$2').trim()
}

function stripMarkdownLinks(text: string): string {
  return text.replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
}

export function protocolMessageRow(text: string): ActivityRowModel | null {
  if (isFinalReply(text)) {
    const parsed = parseProposal(text)
    const rawTitle = (parsed.summary || text.split(LINE_BREAK, 1)[0] || 'Plano').trim()
    const title = stripMarkdownLinks(rawTitle).split(LINE_BREAK, 1)[0]?.trim() || 'Plano'
    return {
      id: 'protocol-plan-ready',
      icon: 'artifact',
      verb: 'Plano pronto',
      chip: { label: title, title, icon: 'document' },
      failed: false,
    }
  }

  for (const { prefix, verb, quoted } of PROTOCOL_PREFIXES) {
    if (!text.startsWith(prefix)) continue
    const firstLine = text.split(LINE_BREAK, 1)[0] ?? ''
    const title = extractProtocolTitle(firstLine, prefix, quoted)
    return {
      id: 'protocol-prompt',
      icon: 'artifact',
      verb,
      chip: { label: title, title, icon: 'generic' },
      items: verb === 'Triagem' ? undefined : text.split(LINE_BREAK).filter((line) => line.trim() !== ''),
      failed: false,
    }
  }

  return null
}

// Confirmações que as ferramentas do Vokia devolvem em texto ("Card X criado: título."); só linhas inteiras casam.
const CONFIRMATION_LINE =
  /^(Card|Etapa|Artefato) (\S+) (criado|atualizado|removido|arquivado|restaurado|movido para .+?|já está em .+?)(?:: (.+?))?\.$/
const CONFIRMATION_ID_PREFIX = 'confirmation-'
const CONFIRMATION_ICON: Record<string, RowIconKind> = { Card: 'backlog', Etapa: 'backlog', Artefato: 'artifact' }

export interface VokiaConfirmations {
  rows: ActivityRowModel[]
  rest: string
}

export function splitVokiaConfirmations(text: string): VokiaConfirmations {
  const rows: ActivityRowModel[] = []
  const rest: string[] = []
  for (const line of text.split(LINE_BREAK)) {
    const match = CONFIRMATION_LINE.exec(line.trim())
    if (!match) {
      rest.push(line)
      continue
    }
    const [, subject, id, action, title] = match
    rows.push({
      id: `${CONFIRMATION_ID_PREFIX}${rows.length}`,
      icon: CONFIRMATION_ICON[subject],
      verb: `${subject} ${action}`,
      chip: { label: title ?? id, title: id, icon: 'generic' },
      failed: false,
    })
  }
  return rows.length === 0 ? { rows, rest: text } : { rows, rest: rest.join(LINE_BREAK).trim() }
}

export function activityRows(tools: readonly ToolActivity[], titleOf: CardTitleOf, nameOf: AgentNameOf): ActivityRowModel[] {
  return groupRepeatedRows(buildRows(tools, titleOf, nameOf))
}

function buildRows(tools: readonly ToolActivity[], titleOf: CardTitleOf, nameOf: AgentNameOf): ActivityRowModel[] {
  // Criação conta desde a entrada da ferramenta: a linha nasce uma vez e só cresce com o contador.
  const created = tools.filter((tool) => tool.detail?.kind === 'created')
  return tools.flatMap((tool) => {
    const icon = ROW_ICON[tool.category]
    if (tool.detail?.kind === 'created') return tool === created[0] ? [taskCreatedRow(created)] : []
    if (tool.detail?.kind === 'started') return startedRow(tool, tool.detail, titleOf)
    if (tool.detail?.kind === 'updated') return [taskUpdatedRow(tool, tool.detail, created)]
    if (tool.detail?.kind === 'read') return [readRow(tool, tool.detail, icon)]
    if (tool.detail?.kind === 'found') return [foundRow(tool, tool.detail, icon)]
    if (tool.detail?.kind === 'command') return [commandRow(tool, tool.detail, nameOf)]
    const delegation = delegationOf(tool)
    if (delegation) return [delegationRow(tool, delegation)]
    return [defaultRow(tool, icon, tool.status === 'failed')]
  })
}

// Só linhas simples se fundem: as que têm lista ou prévia carregam conteúdo próprio que o contador esconderia.
function repeatKey(row: ActivityRowModel): string | undefined {
  if (row.items !== undefined || row.preview !== undefined) return undefined
  return JSON.stringify([row.icon, row.verb, row.chip?.label, row.chip?.title, row.chip?.icon, row.text, row.meta, row.command, row.failed])
}

// Funde linhas consecutivas idênticas (mesmo verbo, alvo e status) em uma, na posição da primeira, com o total em `count`.
export function groupRepeatedRows(rows: readonly ActivityRowModel[]): ActivityRowModel[] {
  const grouped: ActivityRowModel[] = []
  let previousKey: string | undefined
  for (const row of rows) {
    const key = repeatKey(row)
    const last = grouped[grouped.length - 1]
    if (key !== undefined && key === previousKey && last) {
      last.count = (last.count ?? 1) + 1
    } else {
      grouped.push({ ...row })
    }
    previousKey = key
  }
  return grouped
}
