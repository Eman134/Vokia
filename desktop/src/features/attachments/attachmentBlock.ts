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

const PREVIEW_IMAGE_EXTS = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp'])
const ALL_IMAGE_EXTS = new Set([...PREVIEW_IMAGE_EXTS, 'svg', 'bmp', 'ico'])
const CODE_EXTS = new Set([
  'ts', 'tsx', 'js', 'jsx', 'vue', 'rs', 'py', 'go', 'java', 'c', 'cpp', 'h', 'hpp',
  'css', 'scss', 'html', 'json', 'yaml', 'yml', 'toml', 'sh', 'sql', 'xml',
])
const DOC_EXTS = new Set(['pdf', 'doc', 'docx', 'odt', 'rtf'])
const TEXT_EXTS = new Set(['txt', 'md', 'csv', 'log'])

const ICON_IMAGE = 'M4 6a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6Zm4.5 3.5a1.5 1.5 0 1 0 0 .01M6 17l4-4 3 3 2-2 3 3'
const ICON_CODE = 'm8 9-3 3 3 3m8-6 3 3-3 3m-5 2 2-10'
const ICON_DOC = 'M7 3h7l5 5v13a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Zm7 0v5h5M9 13h6M9 17h4'
const ICON_TEXT = 'M6 4h12v16H6V4Zm3 5h6M9 13h6M9 17h4'
const ICON_GENERIC = 'M7 3h7l5 5v13a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Zm7 0v5h5'

// Caminho absoluto na pasta de anexos do Vokia: .../attachments/<project_id>/<hex_id>/<arquivo>
const INLINE_ATTACHMENT_PATH_RE = /`?((?:[A-Za-z]:[\\/]|\/)[^\r\n`"'<>]*?[\\/]attachments[\\/]\d+[\\/][0-9a-fA-F]+[\\/][^\\/\s`"'<>]+)`?/g
const CORRUPTED_DRIVE_FILE_RE = /\b[A-Za-z]:([^\s\\/:]+\.(?:png|jpe?g|gif|webp|svg|bmp|ico))\b/gi
const TRAILING_PUNCTUATION_RE = /[.,;:)]+$/

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

export function isImageAttachmentPath(pathOrName: string): boolean {
  const ext = pathOrName.split('.').pop()?.toLowerCase() ?? ''
  return PREVIEW_IMAGE_EXTS.has(ext)
}

export function attachmentIconPath(name: string): string {
  const ext = name.split('.').pop()?.toLowerCase() ?? ''
  if (ALL_IMAGE_EXTS.has(ext)) return ICON_IMAGE
  if (CODE_EXTS.has(ext)) return ICON_CODE
  if (DOC_EXTS.has(ext)) return ICON_DOC
  if (TEXT_EXTS.has(ext)) return ICON_TEXT
  return ICON_GENERIC
}

function fileNameFromPath(filePath: string): string {
  const parts = filePath.split(/[\\/]/)
  return parts[parts.length - 1] || filePath
}

/** Extrai o bloco estruturado de anexos e também caminhos inline de anexos no texto, ocultando o caminho completo. */
export function extractAttachments(text: string): AttachmentsSplit {
  const base = splitAttachments(text)
  const attachments = [...base.attachments]
  const seen = new Set(attachments.map((item) => item.path))

  const cleanedLines = base.text.split('\n').map((line) => {
    const replaced = line
      .replace(INLINE_ATTACHMENT_PATH_RE, (fullMatch, rawPath: string) => {
        const cleanPath = rawPath.replace(TRAILING_PUNCTUATION_RE, '')
        const trailing = rawPath.slice(cleanPath.length)
        const name = fileNameFromPath(cleanPath)
        if (!seen.has(cleanPath)) {
          seen.add(cleanPath)
          attachments.push({
            name,
            label: isImageAttachmentPath(name) ? 'imagem' : 'arquivo',
            size: '',
            path: cleanPath,
          })
        }
        if (line.trim() === fullMatch.trim()) return ''
        return `${name}${trailing}`
      })
      .replace(CORRUPTED_DRIVE_FILE_RE, '$1')
    return replaced
  })

  const cleanedText = cleanedLines
    .filter((line, index, arr) => !(line === '' && arr[index - 1] === ''))
    .join('\n')
    .trim()

  return { text: cleanedText, attachments }
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
