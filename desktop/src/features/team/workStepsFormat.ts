import type { ToolActivity, ToolStatus, TurnActivity } from './events'
import type { HistoryWorkStepEntry } from './teamApi'

const WORK_STEPS_OPEN = '[work steps]'
const WORK_STEPS_CLOSE = '[end work steps]'
const OMITTED_PATTERN = /^\((\d+)\s+passos?\s+anteriores?\s+omitidos?\)$/i
const MAX_LOCAL_OUTPUT_TAIL_CHARS = 240

export interface ParsedWorkSteps {
  steps: string[]
  omittedCount: number
  provider?: string
}

function statusLabel(status: ToolStatus): string {
  switch (status) {
    case 'running':
      return 'running'
    case 'ok':
      return 'ok'
    case 'failed':
      return 'failed'
  }
}

function collapseWhitespace(text: string): string {
  return text.trim().replace(/\s+/g, ' ')
}

function tailText(raw: string, maxChars: number): string {
  const trimmed = raw.trim()
  if (trimmed.length <= maxChars) return trimmed
  return `…${trimmed.slice(trimmed.length - maxChars)}`
}

function formatLocalToolStep(
  tool: ToolActivity,
  pendingFiles: { status: string; paths: string[] } | null,
  steps: string[],
): { status: string; paths: string[] } | null {
  const flushFiles = () => {
    if (pendingFiles && pendingFiles.paths.length > 0) {
      steps.push(`files (${pendingFiles.status}): ${pendingFiles.paths.join(', ')}`)
    }
  }

  const status = statusLabel(tool.status)

  if (tool.category === 'edit') {
    const path = collapseWhitespace(tool.target)
    if (!path) return pendingFiles
    const entry = `edit ${path}`
    if (pendingFiles && pendingFiles.status === status) {
      pendingFiles.paths.push(entry)
      return pendingFiles
    }
    flushFiles()
    return { status, paths: [entry] }
  }

  if (tool.category === 'read' || tool.category === 'list' || tool.category === 'search') {
    return pendingFiles
  }

  flushFiles()

  if (tool.category === 'command') {
    const cmd = collapseWhitespace(tool.target)
    if (!cmd) return null
    let line = `$ ${cmd} (${status})`
    if (tool.result) {
      const tail = tailText(tool.result, MAX_LOCAL_OUTPUT_TAIL_CHARS)
      if (tail) {
        const indented = tail
          .split('\n')
          .slice(-3)
          .map((l) => `  ${l}`)
          .join('\n')
        line = `${line}\n${indented}`
      }
    }
    steps.push(line)
    return null
  }

  if (tool.category === 'mcp') {
    const name = collapseWhitespace(tool.name)
    if (name) steps.push(`tool ${name} (${status})`)
    return null
  }

  if (tool.category === 'app') {
    const commandName =
      tool.detail?.kind === 'command' && tool.detail.command.trim() !== ''
        ? tool.detail.command
        : tool.target.trim() || tool.name
    const clean = collapseWhitespace(commandName)
    if (clean) steps.push(`tool vokia/${clean} (${status})`)
    return null
  }

  if (tool.category === 'web') {
    if (tool.name.toLowerCase().includes('search')) {
      const query = collapseWhitespace(tool.target)
      if (query) steps.push(`searched: ${query}`)
    } else {
      const clean = collapseWhitespace(tool.name)
      if (clean) steps.push(`tool web/${clean} (${status})`)
    }
    return null
  }

  const clean = collapseWhitespace(tool.name)
  if (clean) steps.push(`tool ${clean} (${status})`)
  return null
}

export function workStepsFromActivity(activity: TurnActivity | undefined): ParsedWorkSteps {
  if (!activity || activity.tools.length === 0) {
    return { steps: [], omittedCount: 0 }
  }
  const steps: string[] = []
  let pendingFiles: { status: string; paths: string[] } | null = null
  for (const tool of activity.tools) {
    pendingFiles = formatLocalToolStep(tool, pendingFiles, steps)
  }
  if (pendingFiles && pendingFiles.paths.length > 0) {
    steps.push(`files (${pendingFiles.status}): ${pendingFiles.paths.join(', ')}`)
  }
  return { steps, omittedCount: 0 }
}

export function parseWorkStepsBlock(entry: HistoryWorkStepEntry): ParsedWorkSteps {
  const lines = entry.block.split(/\r?\n/)
  const steps: string[] = []
  let omittedCount = entry.omittedCount
  let currentStep: string[] = []

  const flushStep = () => {
    if (currentStep.length > 0) {
      steps.push(currentStep.join('\n'))
      currentStep = []
    }
  }

  for (const rawLine of lines) {
    const trimmed = rawLine.trim()
    if (trimmed === WORK_STEPS_OPEN || trimmed === WORK_STEPS_CLOSE) continue
    if (trimmed === '' && currentStep.length === 0) continue

    const omittedMatch = trimmed.match(OMITTED_PATTERN)
    if (omittedMatch) {
      const parsed = Number.parseInt(omittedMatch[1] ?? '0', 10)
      if (!Number.isNaN(parsed) && parsed > omittedCount) {
        omittedCount = parsed
      }
      continue
    }

    if (rawLine.startsWith('  ') && currentStep.length > 0) {
      currentStep.push(rawLine)
    } else {
      flushStep()
      currentStep.push(rawLine)
    }
  }
  flushStep()

  return {
    steps,
    omittedCount,
    provider: entry.provider,
  }
}

export function pickClosestWorkStepEntry(
  entries: readonly HistoryWorkStepEntry[],
  messageTime?: number,
): HistoryWorkStepEntry | undefined {
  if (entries.length === 0) return undefined
  if (messageTime === undefined) return entries[0]
  let best = entries[0]
  let bestDiff = Math.abs(best.createdAt - messageTime)
  for (let i = 1; i < entries.length; i += 1) {
    const candidate = entries[i]
    const diff = Math.abs(candidate.createdAt - messageTime)
    if (diff < bestDiff) {
      best = candidate
      bestDiff = diff
    }
  }
  return best
}
