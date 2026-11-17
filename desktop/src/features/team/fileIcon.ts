export type FileIconKind = 'document' | 'sheet' | 'code' | 'generic'

const EXTENSION_KIND: Readonly<Record<string, FileIconKind>> = {
  md: 'document',
  txt: 'document',
  pdf: 'document',
  doc: 'document',
  docx: 'document',
  csv: 'sheet',
  xls: 'sheet',
  xlsx: 'sheet',
  ts: 'code',
  tsx: 'code',
  js: 'code',
  vue: 'code',
  rs: 'code',
  py: 'code',
  json: 'code',
  css: 'code',
  html: 'code',
}

// Extensão só conta depois do último separador: "pasta.x/arquivo" não tem extensão.
const EXTENSION = /\.([^./\\]+)$/

export function fileIconOf(name: string): FileIconKind {
  const extension = EXTENSION.exec(name)?.[1]?.toLowerCase()
  return extension === undefined ? 'generic' : (EXTENSION_KIND[extension] ?? 'generic')
}
