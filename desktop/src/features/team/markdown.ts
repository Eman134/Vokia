// Markdown mínimo para as mensagens do chat. Todo texto é escapado antes de virar HTML,
// então o resultado é seguro para v-html: nada digitado pelo usuário vira tag ou atributo.

const HTML_ESCAPES: Record<string, string> = {
  '&': '&amp;',
  '<': '&lt;',
  '>': '&gt;',
  '"': '&quot;',
  "'": '&#39;',
}

const FENCE_OPEN = /^```\s*([\w+#.-]*)\s*$/
const FENCE_CLOSE = /^```\s*$/
const HEADING = /^(#{1,3})\s+(.+?)\s*#*\s*$/
const RULE = /^\s*([-*_])(\s*\1){2,}\s*$/
const QUOTE = /^\s{0,3}>\s?(.*)$/
const UL_ITEM = /^\s*[-*+]\s+(.*)$/
const OL_ITEM = /^\s*\d+[.)]\s+(.*)$/
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

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function startsBlock(line: string): boolean {
  return (
    FENCE_OPEN.test(line) ||
    HEADING.test(line) ||
    RULE.test(line) ||
    QUOTE.test(line) ||
    UL_ITEM.test(line) ||
    OL_ITEM.test(line)
  )
}

function formatInline(raw: string, mentions: RegExp | null): string {
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
  if (mentions !== null) {
    text = text.replace(mentions, '<span class="text-accent">$&</span>')
  }
  return text.replace(PLACEHOLDER, (_, index: string) => stash[Number(index)])
}

function renderBlocks(lines: readonly string[], mentions: RegExp | null): string {
  const html: string[] = []
  let i = 0

  while (i < lines.length) {
    const line = lines[i]

    const fence = FENCE_OPEN.exec(line)
    if (fence) {
      const code: string[] = []
      i++
      while (i < lines.length && !FENCE_CLOSE.test(lines[i])) {
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
        items.push(`<li>${formatInline(listPattern.exec(lines[i])![1], mentions)}</li>`)
        i++
      }
      html.push(`<${tag}>${items.join('')}</${tag}>`)
      continue
    }

    const paragraph: string[] = []
    while (i < lines.length && lines[i].trim() !== '' && !startsBlock(lines[i])) {
      paragraph.push(formatInline(lines[i], mentions))
      i++
    }
    html.push(`<p>${paragraph.join('<br>')}</p>`)
  }

  return html.join('')
}

/** Converte o texto de uma mensagem em HTML seguro. `mentionNames` são nomes a destacar como `@Nome`. */
export function renderMarkdown(source: string, mentionNames: readonly string[] = []): string {
  const mentions =
    mentionNames.length === 0
      ? null
      : new RegExp(`(?<=^|[\\s>])@(${mentionNames.map(escapeRegExp).join('|')})(?!\\p{L})`, 'giu')
  return renderBlocks(source.replace(/\r\n?/g, '\n').split('\n'), mentions)
}
