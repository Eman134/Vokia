import type { ConsoleEntry, ToolActivity } from '../team/events'
import { currentTurn } from './consoleTurn'

export type LiveTimelineSegment =
  | { kind: 'text'; text: string }
  | { kind: 'tools'; tools: ToolActivity[] }

// Console e activity.tools recebem uma entrada por tool_use na mesma ordem; a sobra (limite MAX_TOOLS ou buffer cortado) vai para o fim.
export function liveTimeline(
  entries: readonly ConsoleEntry[],
  tools: readonly ToolActivity[],
): LiveTimelineSegment[] {
  const segments: LiveTimelineSegment[] = []
  let toolIndex = 0

  function pushText(text: string): void {
    const trimmed = text.trim()
    if (!trimmed) return
    const last = segments.at(-1)
    if (last?.kind === 'text') {
      last.text = `${last.text}\n${trimmed}`
    } else {
      segments.push({ kind: 'text', text: trimmed })
    }
  }

  function pushTool(tool: ToolActivity): void {
    const last = segments.at(-1)
    if (last?.kind === 'tools') {
      last.tools.push(tool)
    } else {
      segments.push({ kind: 'tools', tools: [tool] })
    }
  }

  for (const entry of currentTurn(entries)) {
    if (entry.kind === 'text') {
      pushText(entry.text)
    } else if (entry.kind === 'tool' && toolIndex < tools.length) {
      pushTool(tools[toolIndex])
      toolIndex += 1
    }
  }

  while (toolIndex < tools.length) {
    pushTool(tools[toolIndex])
    toolIndex += 1
  }

  return segments
}
