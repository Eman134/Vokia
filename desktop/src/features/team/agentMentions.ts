import { normalizeName } from './useRoster'

export interface AgentMention<T extends { name: string }> {
  start: number
  end: number
  agent: T
}

export function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

// Os caracteres `< / & #` antes do nome evitam casar dentro de tags e entidades do HTML que o markdown
// gera; o mesmo critério vale para o texto digitado, para as duas telas destacarem igual.
function mentionPattern(name: string): RegExp {
  return new RegExp(`(?<![\\p{L}\\p{N}<\\/&#@])@?${escapeRegExp(normalizeName(name))}(?![\\p{L}\\p{N}])`, 'gu')
}

// Posições valem no texto original: normalizeName troca letra acentuada por uma letra, sem mudar o tamanho.
export function findAgentMentions<T extends { name: string }>(text: string, agents: readonly T[]): AgentMention<T>[] {
  const normalized = normalizeName(text)
  const found = agents.flatMap((agent) =>
    [...normalized.matchAll(mentionPattern(agent.name))].map((match) => ({
      start: match.index,
      end: match.index + match[0].length,
      agent,
    })),
  )
  found.sort((a, b) => a.start - b.start)
  const kept: AgentMention<T>[] = []
  for (const mention of found) {
    if (mention.start >= (kept.at(-1)?.end ?? 0)) kept.push(mention)
  }
  return kept
}
