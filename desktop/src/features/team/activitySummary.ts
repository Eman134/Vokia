import type { ToolActivity, ToolCategory, ToolStatus } from './events'

const MS_PER_SECOND = 1000
const SECONDS_PER_MINUTE = 60
const MINUTES_PER_HOUR = 60
const MIN_THINKING_MS = MS_PER_SECOND
const PATH_SEPARATOR = /[\\/]/
const MINUS = '−'

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

const FILE_CATEGORIES: readonly ToolCategory[] = ['read', 'edit']

export interface ToolRow {
  label: string
  title: string
  changes?: string
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

export function formatDuration(ms: number): string {
  if (ms < MS_PER_SECOND) return `${Math.round(ms)}ms`
  const totalSeconds = Math.round(ms / MS_PER_SECOND)
  const seconds = totalSeconds % SECONDS_PER_MINUTE
  const totalMinutes = Math.floor(totalSeconds / SECONDS_PER_MINUTE)
  if (totalMinutes === 0) return `${seconds}s`
  const minutes = totalMinutes % MINUTES_PER_HOUR
  const hours = Math.floor(totalMinutes / MINUTES_PER_HOUR)
  return hours === 0 ? `${minutes}m ${seconds}s` : `${hours}h ${minutes}m ${seconds}s`
}

export function thinkingLabel(thinkingMs: number): string {
  return `Pensou por ${formatDuration(Math.max(thinkingMs, MIN_THINKING_MS))}`
}

function verbFor(category: ToolCategory, status: ToolStatus): string {
  const verbs = ROW_VERBS[category]
  if (status === 'failed') return verbs.failed
  return status === 'running' ? verbs.running : verbs.done
}

function shownTarget(tool: ToolActivity): string {
  if (!FILE_CATEGORIES.includes(tool.category)) return tool.target
  return tool.target.split(PATH_SEPARATOR).pop() || tool.target
}

export function toolRowLabel(tool: ToolActivity): ToolRow {
  const verb = verbFor(tool.category, tool.status)
  const target = shownTarget(tool)
  const showChanges = tool.status === 'ok' && tool.added !== undefined && tool.removed !== undefined
  return {
    label: target === '' ? verb : `${verb} ${target}`,
    title: tool.target,
    changes: showChanges ? `+${tool.added} ${MINUS}${tool.removed}` : undefined,
  }
}
