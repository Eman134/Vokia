import type { ConsoleEntry } from '../team/events'

export type DiffTone = 'plain' | 'add' | 'remove'
export type NoteTone = 'ok' | 'danger' | 'warn'

export interface TerminalPrompt {
  time: number
  text: string
}

export interface ResultLine {
  text: string
  tone: DiffTone
}

export type TerminalRow =
  | { type: 'prompt'; key: string; text: string }
  | { type: 'tool'; key: string; name: string; argument: string }
  | { type: 'agent'; key: string; text: string }
  | { type: 'result'; key: string; lines: ResultLine[]; failed: boolean }
  | { type: 'diff'; key: string; lines: string[] }
  | { type: 'note'; key: string; text: string; tone: NoteTone }

const WRITE_TOOLS = ['Write', 'Edit']
const WRITE_TOOL = 'Write'
const READ_TOOL = 'Read'
const DIFF_TOOL = 'Bash'
const ADDED_PREFIX = '+'
const REMOVED_PREFIX = '-'
const ADDED_FILE_MARK = '+++'
const REMOVED_FILE_MARK = '---'
const NOTE_TEXT: Record<'permission' | 'done' | 'failed', { mark: string; tone: NoteTone }> = {
  permission: { mark: '?', tone: 'warn' },
  done: { mark: '✓', tone: 'ok' },
  failed: { mark: '✕', tone: 'danger' },
}

function diffTone(line: string): DiffTone {
  if (line.startsWith(ADDED_PREFIX) && !line.startsWith(ADDED_FILE_MARK)) return 'add'
  if (line.startsWith(REMOVED_PREFIX) && !line.startsWith(REMOVED_FILE_MARK)) return 'remove'
  return 'plain'
}

function resultLines(text: string, mode: 'diff' | 'plain'): ResultLine[] {
  return text.split('\n').map((line) => ({ text: line, tone: mode === 'diff' ? diffTone(line) : 'plain' }))
}

function summary(key: string, text: string): TerminalRow {
  return { type: 'result', key, lines: [{ text, tone: 'plain' }], failed: false }
}

// O buffer não guarda o old_string do Edit: só as linhas adicionadas são conhecidas.
function writeSummary(tool: string, path: string, added: number): string {
  return tool === WRITE_TOOL ? `Wrote ${added} lines to ${path}` : `Updated ${path} with ${added} additions`
}

// O corpo escrito (Write/Edit) vem do buffer como a primeira saída logo após a ferramenta; só ele é "adicionado".
export function buildRows(entries: readonly ConsoleEntry[], prompts: readonly TerminalPrompt[]): TerminalRow[] {
  const rows: TerminalRow[] = []
  const pending = [...prompts].sort((first, second) => first.time - second.time)
  let lastTool = ''
  let lastArgument = ''
  let bodyPending = false

  function flushPrompts(until: number): void {
    while (pending.length > 0 && pending[0].time <= until) {
      const prompt = pending.shift() as TerminalPrompt
      rows.push({ type: 'prompt', key: `p${prompt.time}-${rows.length}`, text: prompt.text })
    }
  }

  for (const entry of entries) {
    flushPrompts(entry.time)
    const key = `e${entry.seq}`
    switch (entry.kind) {
      case 'start':
        break
      case 'text':
        rows.push({ type: 'agent', key, text: entry.text })
        break
      case 'tool': {
        const [name, ...rest] = entry.text.split(' ')
        lastTool = name
        lastArgument = rest.join(' ')
        bodyPending = WRITE_TOOLS.includes(name)
        rows.push({ type: 'tool', key, name, argument: lastArgument })
        break
      }
      case 'output': {
        if (bodyPending) {
          bodyPending = false
          const added = entry.text.split('\n')
          rows.push(summary(`${key}-summary`, writeSummary(lastTool, lastArgument, added.length)))
          rows.push({ type: 'diff', key, lines: added.map((line) => `${ADDED_PREFIX} ${line}`) })
        } else if (lastTool === READ_TOOL) {
          rows.push(summary(key, `Read ${entry.text.split('\n').length} lines`))
        } else {
          const mode = lastTool === DIFF_TOOL ? 'diff' : 'plain'
          rows.push({ type: 'result', key, lines: resultLines(entry.text, mode), failed: false })
        }
        break
      }
      case 'error':
        rows.push({ type: 'result', key, lines: resultLines(entry.text, 'plain'), failed: true })
        break
      default:
        rows.push({ type: 'note', key, text: `${NOTE_TEXT[entry.kind].mark} ${entry.text}`, tone: NOTE_TEXT[entry.kind].tone })
    }
  }
  flushPrompts(Number.POSITIVE_INFINITY)
  return rows
}
