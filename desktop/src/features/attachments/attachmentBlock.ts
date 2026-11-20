// Espelham attachments/mod.rs: o Rust grava o bloco com estes textos exatos, então qualquer diferença quebra o reconhecimento.
export const ATTACHMENTS_HEADER = 'Anexos do usuário:'
export const ATTACHMENTS_FOOTER =
  'Leia um anexo com a ferramenta Read só quando ele for necessário. Ao delegar, inclua no pedido o caminho dos anexos de que o outro agente precisar.'

const EMPTY_LINE = ''
const ITEM_PREFIX = '- '
const PATH_SEPARATOR = '): '
const NAME_OPEN = ' ('
const DETAILS_SEPARATOR = ', '
const BYTES_PER_KB = 1024
export const BYTES_PER_MB = BYTES_PER_KB * BYTES_PER_KB

export interface Attachment {
  name: string
  label: string
  size: string
  path: string
}

export interface AttachmentsSplit {
  text: string
  attachments: Attachment[]
}

export function splitAttachments(text: string): AttachmentsSplit {
  const lines = text.split('\n')
  // A última ocorrência é a do bloco acrescentado ao fim do pedido; o texto do usuário pode citar o cabeçalho antes.
  const header = lines.lastIndexOf(ATTACHMENTS_HEADER)
  if (header === -1) return withoutAttachments(text)

  const footer = lines.reduce((last, line, index) => (line.trim() === EMPTY_LINE ? last : index), -1)
  if (footer <= header + 1 || lines[footer] !== ATTACHMENTS_FOOTER) return withoutAttachments(text)

  const attachments = parseItems(lines.slice(header + 1, footer))
  if (attachments === null) return withoutAttachments(text)

  return { text: lines.slice(0, header).join('\n').trimEnd(), attachments }
}

export function formatSize(bytes: number): string {
  if (bytes < BYTES_PER_KB) return `${bytes} B`
  if (bytes < BYTES_PER_MB) return `${Math.floor(bytes / BYTES_PER_KB)} KB`
  return `${(bytes / BYTES_PER_MB).toFixed(1).replace('.', ',')} MB`
}

function withoutAttachments(text: string): AttachmentsSplit {
  return { text, attachments: [] }
}

function parseItems(lines: string[]): Attachment[] | null {
  const attachments: Attachment[] = []
  for (const line of lines) {
    const attachment = parseItem(line)
    if (attachment === null) return null
    attachments.push(attachment)
  }
  return attachments
}

function parseItem(line: string): Attachment | null {
  if (!line.startsWith(ITEM_PREFIX)) return null
  const body = line.slice(ITEM_PREFIX.length)

  const pathAt = body.lastIndexOf(PATH_SEPARATOR)
  if (pathAt === -1) return null
  const path = body.slice(pathAt + PATH_SEPARATOR.length)

  const head = body.slice(0, pathAt + 1)
  if (!head.endsWith(')')) return null
  // Nome pode conter " (", por isso vale o último que abre os detalhes.
  const nameAt = head.lastIndexOf(NAME_OPEN)
  if (nameAt <= 0) return null
  const name = head.slice(0, nameAt)

  const details = head.slice(nameAt + NAME_OPEN.length, -1)
  const separatorAt = details.indexOf(DETAILS_SEPARATOR)
  if (separatorAt === -1) return null
  const label = details.slice(0, separatorAt)
  const size = details.slice(separatorAt + DETAILS_SEPARATOR.length)

  if (!label || !size || !path) return null
  return { name, label, size, path }
}
