import { normalizeName, useRoster } from './useRoster'
import type { Agent } from './events'
import { escapeRegExp } from './agentMentions'
import type { BacklogItem } from '../backlog/backlogTypes'

export const MENTION_TRIGGER = '@'
export const TASK_MENTION_TRIGGER = '#'

export interface TriggerMatch {
  start: number
  query: string
}

export interface MentionSpan {
  start: number
  end: number
  agent: Agent
}

export interface TaskMentionSpan {
  start: number
  end: number
  itemId: string
  label: string
}

export function findTrigger(text: string, caret: number, trigger: string): TriggerMatch | null {
  const match = new RegExp(`(?:^|\\s)${escapeRegExp(trigger)}(\\p{L}*)$`, 'u').exec(text.slice(0, caret))
  if (match === null) {
    return null
  }
  const query = match[1]
  return { start: match.index + match[0].length - query.length - trigger.length, query }
}

export function mentionSpans(text: string, confirmed: ReadonlySet<Agent>): MentionSpan[] {
  const normalizedText = normalizeName(text)
  return useRoster().agents.filter((agent) => confirmed.has(agent.id))
    .flatMap((agent) =>
      [...normalizedText.matchAll(new RegExp(`(?<=^|\\s)${MENTION_TRIGGER}${normalizeName(agent.name)}(?!\\p{L})`, 'gu'))].map((match) => ({
        start: match.index,
        end: match.index + match[0].length,
        agent: agent.id,
      })),
    )
    .sort((a, b) => a.start - b.start)
}

export interface MentionSegment {
  text: string
  agent: Agent | null
}

export function mentionSegments(text: string, spans: readonly MentionSpan[]): MentionSegment[] {
  const segments: MentionSegment[] = []
  let cursor = 0
  for (const span of spans) {
    segments.push({ text: text.slice(cursor, span.start), agent: null })
    segments.push({ text: text.slice(span.start, span.end), agent: span.agent })
    cursor = span.end
  }
  segments.push({ text: text.slice(cursor), agent: null })
  return segments
}

export function replaceTrigger(
  text: string,
  match: TriggerMatch,
  caret: number,
  token: string,
): { text: string; caret: number } {
  return {
    text: text.slice(0, match.start) + token + text.slice(caret),
    caret: match.start + token.length,
  }
}

export function insertMention(
  text: string,
  match: TriggerMatch,
  caret: number,
  name: string,
): { text: string; caret: number } {
  return replaceTrigger(text, match, caret, `${MENTION_TRIGGER}${name} `)
}

export function findTaskTrigger(text: string, caret: number): TriggerMatch | null {
  const match = new RegExp(`(?:^|\\s)${escapeRegExp(TASK_MENTION_TRIGGER)}([\\p{L}\\p{N}_-]*)$`, 'u').exec(
    text.slice(0, caret),
  )
  if (match === null) {
    return null
  }
  const query = match[1]
  return { start: match.index + match[0].length - query.length - TASK_MENTION_TRIGGER.length, query }
}

export function insertTaskMention(
  text: string,
  match: TriggerMatch,
  caret: number,
  identifier: string,
): { text: string; caret: number } {
  return replaceTrigger(text, match, caret, `${TASK_MENTION_TRIGGER}${identifier} `)
}

export function findTaskMentions(text: string, items: readonly BacklogItem[]): TaskMentionSpan[] {
  if (items.length === 0 || !text.includes(TASK_MENTION_TRIGGER)) {
    return []
  }
  const byKey = new Map<string, { itemId: string; label: string }>()
  for (const item of items) {
    const label = item.publicId ?? item.id
    byKey.set(item.id.toLowerCase(), { itemId: item.id, label })
    if (item.publicId) {
      byKey.set(item.publicId.toLowerCase(), { itemId: item.id, label })
    }
  }
  const spans: TaskMentionSpan[] = []
  const regex = new RegExp(`(?<=^|\\s)${escapeRegExp(TASK_MENTION_TRIGGER)}([\\p{L}\\p{N}_-]+)(?![\\p{L}\\p{N}_-])`, 'gu')
  for (const match of text.matchAll(regex)) {
    const key = match[1].toLowerCase()
    const resolved = byKey.get(key)
    if (resolved) {
      spans.push({
        start: match.index,
        end: match.index + match[0].length,
        itemId: resolved.itemId,
        label: resolved.label,
      })
    }
  }
  return spans
}

export type HighlightSegment =
  | { kind: 'text'; text: string }
  | { kind: 'agent'; text: string; agent: Agent }
  | { kind: 'task'; text: string; itemId: string }

export function composerHighlightSegments(
  text: string,
  agentSpans: readonly MentionSpan[],
  taskSpans: readonly TaskMentionSpan[],
): HighlightSegment[] {
  type Combined =
    | { kind: 'agent'; start: number; end: number; agent: Agent }
    | { kind: 'task'; start: number; end: number; itemId: string }

  const combined: Combined[] = [
    ...agentSpans.map((span) => ({ kind: 'agent' as const, start: span.start, end: span.end, agent: span.agent })),
    ...taskSpans.map((span) => ({ kind: 'task' as const, start: span.start, end: span.end, itemId: span.itemId })),
  ].sort((a, b) => a.start - b.start)

  const segments: HighlightSegment[] = []
  let cursor = 0
  for (const span of combined) {
    if (span.start < cursor) continue
    if (span.start > cursor) {
      segments.push({ kind: 'text', text: text.slice(cursor, span.start) })
    }
    if (span.kind === 'agent') {
      segments.push({ kind: 'agent', text: text.slice(span.start, span.end), agent: span.agent })
    } else {
      segments.push({ kind: 'task', text: text.slice(span.start, span.end), itemId: span.itemId })
    }
    cursor = span.end
  }
  if (cursor < text.length || segments.length === 0) {
    segments.push({ kind: 'text', text: text.slice(cursor) })
  }
  return segments
}
