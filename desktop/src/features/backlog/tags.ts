import type { MarkerTone } from '../../ui/UiMarker.vue'

const KNOWN_LABEL: Record<string, string> = { ui: 'interface', divida: 'dívida técnica', cliente: 'pedido de cliente' }
const KNOWN_COLOR: Record<string, string> = {
  bug: 'danger',
  urgente: 'warn',
  ui: 'feature-blue',
  backend: 'feature-green',
  ideia: 'feature-purple',
  divida: 'muted',
  cliente: 'client',
}
// Só há quatro marca-textos: as cores de etiqueta que não têm par direto caem no mais próximo.
const MARKER_TONE: Record<string, MarkerTone> = {
  danger: 'pink',
  client: 'pink',
  warn: 'yellow',
  'feature-orange': 'yellow',
  muted: 'yellow',
  'feature-blue': 'blue',
  'feature-purple': 'blue',
  accent: 'blue',
  'feature-green': 'green',
}
const FREE_COLORS = ['feature-blue', 'feature-green', 'feature-purple', 'feature-orange', 'client', 'accent'] as const

export const TAG_MAX_LENGTH = 24
export const SUGGESTED_TAGS: readonly string[] = ['bug', 'urgente', 'ui', 'backend', 'ideia', 'divida', 'cliente']

export function normalizeTag(raw: string): string | null {
  const text = raw.trim().replace(/\s+/g, ' ').toLowerCase()
  return text === '' || text.length > TAG_MAX_LENGTH ? null : text
}

export function tagLabel(tag: string): string {
  return KNOWN_LABEL[tag] ?? tag
}

export function tagColor(tag: string): string {
  return KNOWN_COLOR[tag] ?? FREE_COLORS[[...tag].reduce((sum, char) => sum + char.charCodeAt(0), 0) % FREE_COLORS.length]
}

export function tagMarkerTone(tag: string): MarkerTone {
  return MARKER_TONE[tagColor(tag)] ?? 'yellow'
}
