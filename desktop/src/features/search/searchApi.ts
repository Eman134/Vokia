import { invoke } from '@tauri-apps/api/core'

const GLOBAL_SEARCH_COMMAND = 'global_search'
const MARK_OPEN = '<mark>'
const MARK_CLOSE = '</mark>'

export type SearchKind =
  | 'projects'
  | 'agents'
  | 'cards'
  | 'plans'
  | 'artifacts'
  | 'memories'
  | 'messages'

export interface SearchHit {
  kind: SearchKind
  id: string
  title: string
  subtitle?: string
  snippet?: string
  projectId?: number
  projectName?: string
  itemId?: string
  channelId?: number
  fromAgent?: string
  toAgent?: string
  artifactKind?: string
  column?: string
  timestamp?: number
  score: number
}

export interface SearchGroup {
  kind: SearchKind
  total: number
  hits: SearchHit[]
  nextCursor?: string
}

export interface GlobalSearchResponse {
  query: string
  total: number
  groups: SearchGroup[]
}

export interface GlobalSearchParams {
  query: string
  projectId?: number | null
  kinds?: SearchKind[]
  limit?: number
  cursor?: string
  offset?: number
}

export interface SnippetSegment {
  text: string
  highlight: boolean
}

export function globalSearch(params: GlobalSearchParams): Promise<GlobalSearchResponse> {
  return invoke<GlobalSearchResponse>(GLOBAL_SEARCH_COMMAND, {
    query: params.query,
    projectId: params.projectId ?? null,
    kinds: params.kinds ?? null,
    limit: params.limit ?? null,
    cursor: params.cursor ?? null,
    offset: params.offset ?? null,
  })
}

export function splitHighlightedSnippet(raw: string): SnippetSegment[] {
  const segments: SnippetSegment[] = []
  let rest = raw
  while (rest.length > 0) {
    const openAt = rest.indexOf(MARK_OPEN)
    if (openAt === -1) {
      segments.push({ text: rest, highlight: false })
      break
    }
    if (openAt > 0) {
      segments.push({ text: rest.slice(0, openAt), highlight: false })
    }
    const afterOpen = rest.slice(openAt + MARK_OPEN.length)
    const closeAt = afterOpen.indexOf(MARK_CLOSE)
    if (closeAt === -1) {
      segments.push({ text: afterOpen, highlight: true })
      break
    }
    if (closeAt > 0) {
      segments.push({ text: afterOpen.slice(0, closeAt), highlight: true })
    }
    rest = afterOpen.slice(closeAt + MARK_CLOSE.length)
  }
  return segments
}
