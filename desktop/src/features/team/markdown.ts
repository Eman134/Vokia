// Markdown mínimo para as mensagens do chat. Todo texto é escapado antes de virar HTML,
// então o resultado é seguro para v-html: nada digitado pelo usuário vira tag ou atributo.

import { findAgentMentions } from './agentMentions'

const HTML_ESCAPES: Record<string, string> = {
  '&': '&amp;',
  '<': '&lt;',
  '>': '&gt;',
  '"': '&quot;',
  "'": '&#39;',
}

interface FenceRules {
  open: RegExp
  close: RegExp
}

// No chat a cerca é estrita, para que "```npm run build```" numa linha continue sendo código inline.
const CHAT_FENCES: FenceRules = { open: /^```\s*([\w+#.-]*)\s*$/, close: /^```\s*$/ }
// Com taskList vale a regra de toggle_check e de artifactChecks.ts: toda linha que começa com ``` (após espaços) abre ou fecha cerca.
const TASK_FENCES: FenceRules = { open: /^\s*```\s*([\w+#.-]*).*$/, close: /^\s*```/ }
const HEADING = /^(#{1,3})\s+(.+?)\s*#*\s*$/
const RULE = /^\s*([-*_])(\s*\1){2,}\s*$/
const QUOTE = /^\s{0,3}>\s?(.*)$/
const UL_ITEM = /^\s*[-*+]\s+(.*)$/
const OL_ITEM = /^\s*\d+[.)]\s+(.*)$/
// Mesma regra de set_artifact_check no backend: só "- [ ]" e "- [x]", com o índice contado na ordem do texto fora de blocos de código.
const TASK_ITEM = /^\s*- \[( |x|X)\](.*)$/
const CODE_SPAN = /`([^`]+)`/g
const LINK = /\[([^\]]+)\]\((https?:\/\/[^\s)]+|mailto:[^\s)]+)\)/g
const BOLD = /\*\*(?=\S)(.+?)(?<=\S)\*\*|__(?=\S)(.+?)(?<=\S)__/g
const STRIKE = /~~(?=\S)(.+?)(?<=\S)~~/g
const ITALIC_STAR = /(^|[^*\w])\*(?=\S)([^*]+?)(?<=\S)\*(?!\*)/g
const ITALIC_UNDER = /(^|[^_\w])_(?=\S)([^_]+?)(?<=\S)_(?![_\w])/g
const PLACEHOLDER = /\u0000(\d+)\u0000/g

export function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (char) => HTML_ESCAPES[char])
}

export interface MentionAgent {
  id: string
  name: string
  icon: string
}

function highlightMentions(html: string, agents: readonly MentionAgent[]): string {
  let out = ''
  let cursor = 0
  for (const { start, end, agent } of findAgentMentions(html, agents)) {
    const shown = html.slice(start, end).replace(/^@/, '')
    out += `${html.slice(cursor, start)}<span data-mention="${agent.id}" class="inline-flex cursor-pointer items-center gap-0.5 align-bottom text-accent">${agent.icon}${shown}</span>`
    cursor = end
  }
  return out + html.slice(cursor)
}

export interface MarkdownOptions {
  taskList?: boolean
}

interface TaskCounter {
  next: number
}

function taskListItem(match: RegExpExecArray, index: number, mentions: readonly MentionAgent[]): string {
  const checked = match[1] === ' ' ? '' : ' checked'
  return `<li class="list-none"><label class="inline-flex cursor-pointer items-start gap-2"><input type="checkbox" class="mt-1 cursor-pointer" data-check-index="${index}"${checked}><span>${formatInline(match[2].trim(), mentions)}</span></label></li>`
}

function startsBlock(line: string, fences: FenceRules): boolean {
  return (
    fences.open.test(line) ||
    HEADING.test(line) ||
    RULE.test(line) ||
    QUOTE.test(line) ||
    UL_ITEM.test(line) ||
    OL_ITEM.test(line)
  )
}

function formatInline(raw: string, mentions: readonly MentionAgent[]): string {
  // Trechos já prontos (código e links) ficam guardados e só voltam no fim,
  // para que negrito, itálico e menções não mexam dentro deles.
  const stash: string[] = []
  const hold = (html: string): string => `\u0000${stash.push(html) - 1}\u0000`

  let text = raw.replace(/\u0000/g, '')
  text = text.replace(CODE_SPAN, (_, code: string) => hold(`<code>${escapeHtml(code)}</code>`))
  text = escapeHtml(text)
  text = text.replace(LINK, (_, label: string, url: string) =>
    hold(`<a href="${url}" target="_blank" rel="noopener noreferrer">${label}</a>`),
  )
  text = text
    .replace(BOLD, (_, a: string | undefined, b: string | undefined) => `<strong>${a ?? b}</strong>`)
    .replace(STRIKE, '<del>$1</del>')
    .replace(ITALIC_STAR, '$1<em>$2</em>')
    .replace(ITALIC_UNDER, '$1<em>$2</em>')
  text = highlightMentions(text, mentions)
  return text.replace(PLACEHOLDER, (_, index: string) => stash[Number(index)])
}

function renderBlocks(lines: readonly string[], mentions: readonly MentionAgent[], tasks: TaskCounter | null = null): string {
  const html: string[] = []
  const fences = tasks === null ? CHAT_FENCES : TASK_FENCES
  let i = 0

  while (i < lines.length) {
    const line = lines[i]

    const fence = fences.open.exec(line)
    if (fence) {
      const code: string[] = []
      i++
      while (i < lines.length && !fences.close.test(lines[i])) {
        code.push(lines[i])
        i++
      }
      i++ // pula a cerca de fechamento (ou o fim do texto)
      const lang = fence[1] ? ` class="language-${escapeHtml(fence[1])}"` : ''
      html.push(`<pre><code${lang}>${escapeHtml(code.join('\n'))}</code></pre>`)
      continue
    }

    if (line.trim() === '') {
      i++
      continue
    }

    const heading = HEADING.exec(line)
    if (heading) {
      const level = heading[1].length + 2
      html.push(`<h${level}>${formatInline(heading[2], mentions)}</h${level}>`)
      i++
      continue
    }

    if (RULE.test(line)) {
      html.push('<hr>')
      i++
      continue
    }

    if (QUOTE.test(line)) {
      const quoted: string[] = []
      while (i < lines.length && QUOTE.test(lines[i])) {
        quoted.push(QUOTE.exec(lines[i])![1])
        i++
      }
      html.push(`<blockquote>${renderBlocks(quoted, mentions)}</blockquote>`)
      continue
    }

    const listPattern = UL_ITEM.test(line) ? UL_ITEM : OL_ITEM.test(line) ? OL_ITEM : null
    if (listPattern !== null) {
      const tag = listPattern === UL_ITEM ? 'ul' : 'ol'
      const items: string[] = []
      while (i < lines.length && listPattern.test(lines[i])) {
        const task = tasks !== null && listPattern === UL_ITEM ? TASK_ITEM.exec(lines[i]) : null
        items.push(
          task && tasks
            ? taskListItem(task, tasks.next++, mentions)
            : `<li>${formatInline(listPattern.exec(lines[i])![1], mentions)}</li>`,
        )
        i++
      }
      html.push(`<${tag}>${items.join('')}</${tag}>`)
      continue
    }

    const paragraph: string[] = []
    while (i < lines.length && lines[i].trim() !== '' && !startsBlock(lines[i], fences)) {
      paragraph.push(formatInline(lines[i], mentions))
      i++
    }
    html.push(`<p>${paragraph.join('<br>')}</p>`)
  }

  return html.join('')
}

/** Converte o texto de uma mensagem em HTML seguro. `agents` são os agentes reais, destacados com o ícone quando citados. */
export function renderMarkdown(
  source: string,
  agents: readonly MentionAgent[] = [],
  options: MarkdownOptions = {},
): string {
  // Com taskList, "- [ ]" vira checkbox numerado (data-check-index); citações não entram na contagem.
  const tasks = options.taskList ? { next: 0 } : null
  return renderBlocks(source.replace(/\r\n?/g, '\n').split('\n'), agents, tasks)
}
