import type { NewRequestDraft } from './backlogTypes'

const TITLE_MAX_CHARS = 80
const ELLIPSIS = '…'
const CONVERSATION_ORIGIN = 'conversa'
const LINK_LABEL_PREFIX = 'Conversa'
const DATE_FORMAT = new Intl.DateTimeFormat('pt-BR', {
  day: '2-digit',
  month: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
  hour12: false,
})

export interface MessageSource {
  text: string
  from: string
  time: number
  author: string
}

function titleOf(text: string): string {
  const firstLine = text.split('\n').find((line) => line.trim() !== '')?.trim() ?? ''
  const chars = [...firstLine]
  return chars.length <= TITLE_MAX_CHARS ? firstLine : `${chars.slice(0, TITLE_MAX_CHARS - 1).join('').trimEnd()}${ELLIPSIS}`
}

export function prefillFromMessage(message: MessageSource): NewRequestDraft {
  const when = DATE_FORMAT.format(message.time).replace(', ', ' ')
  return {
    title: titleOf(message.text),
    description: message.text,
    origin: CONVERSATION_ORIGIN,
    links: [
      {
        id: `conversa:${message.time}:${message.from}`,
        label: `${LINK_LABEL_PREFIX} · ${message.author} · ${when}`,
      },
    ],
  }
}
