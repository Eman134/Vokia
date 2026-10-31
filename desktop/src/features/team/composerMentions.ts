import { AGENTS } from './useTeam'
import type { Agent } from './events'

export const MENTION_TRIGGER = '@'

export interface TriggerMatch {
  start: number
  query: string
}

export interface MentionSpan {
  start: number
  end: number
  agent: Agent
}

const REGEX_SPECIAL_CHARS = /[.*+?^${}()|[\]\\]/g

export function findTrigger(text: string, caret: number, trigger: string): TriggerMatch | null {
  const escaped = trigger.replace(REGEX_SPECIAL_CHARS, '\\$&')
  const match = new RegExp(`(?:^|\\s)${escaped}(\\p{L}*)$`, 'u').exec(text.slice(0, caret))
  if (match === null) {
    return null
  }
  const query = match[1]
  return { start: match.index + match[0].length - query.length - trigger.length, query }
}

export function mentionSpans(text: string, confirmed: ReadonlySet<Agent>): MentionSpan[] {
  return AGENTS.filter((agent) => confirmed.has(agent.id))
    .flatMap((agent) =>
      [...text.matchAll(new RegExp(`(?<=^|\\s)${MENTION_TRIGGER}${agent.name}(?!\\p{L})`, 'giu'))].map((match) => ({
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

export function insertMention(
  text: string,
  match: TriggerMatch,
  caret: number,
  name: string,
): { text: string; caret: number } {
  const token = `${MENTION_TRIGGER}${name} `
  return {
    text: text.slice(0, match.start) + token + text.slice(caret),
    caret: match.start + token.length,
  }
}
