import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { PAGE_LABELS, type AppPage } from '../../shell/railNav'
import { useNavigation } from '../../shell/useNavigation'
import { useRail } from '../../shell/useRail'
import { useBacklog } from '../backlog/useBacklog'
import { MEMORY_FACTS } from '../memory/memoriaSample'
import { useProjects } from '../projects/useProjects'
import { useProviders } from '../providers/useProviders'
import { SETTINGS_SECTIONS } from '../settings/configuracoesSample'
import { useFeed } from '../team/useFeed'
import { normalizeName, useRoster } from '../team/useRoster'
import {
  globalSearch,
  type SearchGroup,
  type SearchHit,
  type SearchKind,
} from './searchApi'

const DEBOUNCE_MS = 120
const DEFAULT_GROUP_LIMIT = 20
const RECENT_STORAGE_KEY = 'vokia:global-search:recent'
const MAX_RECENTS = 6

export type SearchFilterChip =
  | 'all'
  | 'actions'
  | 'projects'
  | 'agents'
  | 'cards'
  | 'artifacts'
  | 'memories'
  | 'messages'

export type PaletteGroupId =
  | 'recent'
  | 'actions'
  | 'projects'
  | 'agents'
  | 'cards'
  | 'artifacts'
  | 'memories'
  | 'messages'

export type PaletteItemTarget =
  | { kind: 'action'; actionId: string }
  | { kind: 'page'; page: AppPage }
  | { kind: 'settings'; section: string }
  | { kind: 'providers'; providerId?: string }
  | { kind: 'project'; projectId: number }
  | { kind: 'agent'; agentId: string }
  | { kind: 'card'; projectId: number; itemId: string }
  | { kind: 'plan'; projectId: number; itemId: string }
  | { kind: 'artifact'; projectId: number; artifactId: string }
  | { kind: 'memory'; text: string }
  | {
      kind: 'message'
      projectId: number
      timestamp: number
      fromAgent: string
      text: string
      channelId?: number
      itemId?: string
    }

export interface PaletteItem {
  key: string
  group: PaletteGroupId
  badge: string
  title: string
  subtitle?: string
  snippet?: string
  projectName?: string
  keywords?: string
  target: PaletteItemTarget
}

export interface PaletteGroup {
  id: PaletteGroupId
  label: string
  items: PaletteItem[]
  total: number
  nextCursor?: string
  backendKind?: SearchKind
}

interface RecentSearchEntry {
  key: string
  badge: string
  title: string
  subtitle?: string
  projectName?: string
  target: PaletteItemTarget
}

export interface FilterChipOption {
  id: SearchFilterChip
  label: string
  prefix?: string
}

export const FILTER_CHIPS: readonly FilterChipOption[] = [
  { id: 'all', label: 'Tudo' },
  { id: 'actions', label: 'Ações', prefix: '>' },
  { id: 'projects', label: 'Projetos' },
  { id: 'agents', label: 'Agentes', prefix: '@' },
  { id: 'cards', label: 'Cards', prefix: '#' },
  { id: 'artifacts', label: 'Planos & Artefatos' },
  { id: 'memories', label: 'Memórias' },
  { id: 'messages', label: 'Mensagens' },
]

export const GROUP_LABELS: Record<PaletteGroupId, string> = {
  recent: 'Recentes',
  actions: 'Ações rápidas e Configurações',
  projects: 'Projetos',
  agents: 'Agentes',
  cards: 'Cards',
  artifacts: 'Planos & Artefatos',
  memories: 'Memórias',
  messages: 'Mensagens',
}

const ARTIFACT_KIND_LABELS: Record<string, string> = {
  plano: 'Plano',
  documento: 'Documento',
  especificacao: 'Especificação',
  checklist: 'Checklist',
  arquitetura: 'Arquitetura',
  decisao: 'Decisão',
}

function readRecents(): RecentSearchEntry[] {
  try {
    const raw = window.localStorage.getItem(RECENT_STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    return Array.isArray(parsed) ? parsed.slice(0, MAX_RECENTS) : []
  } catch {
    return []
  }
}

function writeRecents(entries: RecentSearchEntry[]): void {
  try {
    window.localStorage.setItem(RECENT_STORAGE_KEY, JSON.stringify(entries.slice(0, MAX_RECENTS)))
  } catch {
    // Armazenamento indisponível não impede a busca.
  }
}

function tokenize(text: string): string[] {
  return normalizeName(text)
    .split(/[^a-z0-9]+/)
    .filter(Boolean)
}

function matchesTokens(haystack: string, tokens: string[]): boolean {
  if (tokens.length === 0) return true
  const folded = normalizeName(haystack)
  return tokens.every((token) => folded.includes(token))
}

function highlightMatch(text: string, tokens: string[]): string {
  if (tokens.length === 0 || text.trim() === '') return text
  const folded = normalizeName(text)
  let bestStart = -1
  let bestLen = 0
  for (const token of tokens) {
    const index = folded.indexOf(token)
    if (index !== -1 && (bestStart === -1 || index < bestStart)) {
      bestStart = index
      bestLen = token.length
    }
  }
  if (bestStart === -1) return text
  const chars = [...text]
  const end = Math.min(bestStart + bestLen, chars.length)
  return `${chars.slice(0, bestStart).join('')}<mark>${chars.slice(bestStart, end).join('')}</mark>${chars.slice(end).join('')}`
}

function parseQueryPrefix(raw: string): { prefixChip: SearchFilterChip | null; term: string } {
  const trimmed = raw.trimStart()
  if (trimmed.startsWith('>')) return { prefixChip: 'actions', term: trimmed.slice(1).trim() }
  if (trimmed.startsWith('#')) return { prefixChip: 'cards', term: trimmed.slice(1).trim() }
  if (trimmed.startsWith('@')) return { prefixChip: 'agents', term: trimmed.slice(1).trim() }
  return { prefixChip: null, term: trimmed.trim() }
}

function backendKindsForChip(chip: SearchFilterChip): SearchKind[] {
  switch (chip) {
    case 'actions':
      return []
    case 'projects':
      return ['projects']
    case 'agents':
      return ['agents']
    case 'cards':
      return ['cards']
    case 'artifacts':
      return ['plans', 'artifacts']
    case 'memories':
      return ['memories']
    case 'messages':
      return ['messages']
    case 'all':
      return ['projects', 'agents', 'cards', 'plans', 'artifacts', 'memories', 'messages']
  }
}

export const useGlobalSearch = defineStore('global-search', () => {
  const isOpen = ref(false)
  const query = ref('')
  const selectedChip = ref<SearchFilterChip>('all')
  const currentProjectOnly = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)
  const backendGroups = ref<SearchGroup[]>([])
  const recents = ref<RecentSearchEntry[]>(readRecents())

  const navigation = useNavigation()
  const rail = useRail()
  const projects = useProjects()
  const roster = useRoster()
  const backlog = useBacklog()
  const providers = useProviders()
  const feed = useFeed()

  let debounceTimer: ReturnType<typeof setTimeout> | null = null
  let requestSeq = 0

  const parsed = computed(() => parseQueryPrefix(query.value))
  const effectiveChip = computed<SearchFilterChip>(() => parsed.value.prefixChip ?? selectedChip.value)
  const effectiveTerm = computed(() => parsed.value.term)
  const effectiveTokens = computed(() => tokenize(effectiveTerm.value))
  const hasQuery = computed(() => effectiveTerm.value.length > 0)

  const actionCatalog = computed<PaletteItem[]>(() => {
    const currentPage = navigation.page
    const hasProject = projects.selected !== null

    const quickActions: Array<PaletteItem & { priority: number }> = [
      {
        key: 'action:new-request',
        group: 'actions',
        badge: 'Ação',
        title: 'Novo pedido',
        subtitle: hasProject ? 'Criar um card no backlog do projeto atual' : 'Selecione um projeto para criar um pedido',
        keywords: 'novo pedido criar card task feature backlog',
        target: { kind: 'action', actionId: 'new-request' },
        priority: currentPage === 'backlog' || currentPage === 'conversas' ? 0 : 2,
      },
      {
        key: 'action:new-project',
        group: 'actions',
        badge: 'Ação',
        title: 'Novo projeto',
        subtitle: 'Adicionar pasta local ou repositório GitHub',
        keywords: 'novo projeto pasta repositorio github criar',
        target: { kind: 'page', page: 'novo-projeto' },
        priority: currentPage === 'projetos' ? 0 : 4,
      },
      {
        key: 'action:providers',
        group: 'actions',
        badge: 'Configuração',
        title: 'Provedores e LLMs',
        subtitle: 'Configurar CLIs, modelos padrão e fallback',
        keywords: 'provedores providers llm claude codex gemini opencode modelos cli',
        target: { kind: 'providers' },
        priority: 1,
      },
      {
        key: 'action:toggle-rail',
        group: 'actions',
        badge: 'Ação',
        title: rail.expanded ? 'Recolher menu lateral' : 'Expandir menu lateral',
        subtitle: 'Alternar largura do trilho de navegação',
        keywords: 'menu lateral trilho sidebar recolher expandir',
        target: { kind: 'action', actionId: 'toggle-rail' },
        priority: 8,
      },
    ]

    const pageItems: Array<PaletteItem & { priority: number }> = (
      [
        ['conversas', 'Chat com agentes, canais e threads'],
        ['backlog', 'Quadro kanban e pedidos do projeto'],
        ['canvas', 'Planos, especificações, checklists e decisões'],
        ['projetos', 'Lista e gerenciamento de projetos'],
        ['agentes', 'Elenco de agentes e papéis'],
        ['mapa', 'Camadas, times e fluxo de orquestração'],
        ['estatisticas', 'Consumo de tokens, custos e limites'],
        ['memoria', 'Fatos aprendidos e grafo do projeto'],
        ['config', 'Preferências gerais do Vokia'],
      ] as const
    ).map(([page, subtitle], idx) => ({
      key: `page:${page}`,
      group: 'actions' as const,
      badge: 'Tela',
      title: PAGE_LABELS[page],
      subtitle,
      keywords: `${PAGE_LABELS[page]} ${page} ${subtitle}`,
      target: { kind: 'page' as const, page },
      priority: page === currentPage ? 15 : 5 + idx,
    }))

    const settingsItems: Array<PaletteItem & { priority: number }> = SETTINGS_SECTIONS.map((section, idx) => {
      const itemLabels = section.items
        .map((item) => ('label' in item ? item.label : 'name' in item ? item.name : ''))
        .filter(Boolean)
        .join(' ')
      return {
        key: `settings:${section.title}`,
        group: 'actions' as const,
        badge: 'Configuração',
        title: `Configurações › ${section.title}`,
        subtitle: section.description,
        keywords: `configuracoes settings ${section.title} ${section.description} ${itemLabels}`,
        target: { kind: 'settings' as const, section: section.title },
        priority: 20 + idx,
      }
    })

    return [...quickActions, ...pageItems, ...settingsItems]
      .sort((a, b) => a.priority - b.priority)
      .map(({ priority: _priority, ...item }) => item)
  })

  function filterActions(): PaletteItem[] {
    if (!hasQuery.value) {
      if (effectiveChip.value === 'actions') return actionCatalog.value
      return actionCatalog.value.slice(0, 6)
    }
    const tokens = effectiveTokens.value
    return actionCatalog.value.filter((item) =>
      matchesTokens(`${item.title} ${item.subtitle ?? ''} ${item.keywords ?? ''}`, tokens),
    )
  }

  function localProjects(): PaletteItem[] {
    const selectedId = currentProjectOnly.value ? projects.selected?.id : undefined
    const list = projects.projects.filter((project) => selectedId === undefined || project.id === selectedId)
    if (!hasQuery.value) {
      return list.slice(0, 5).map((project) => ({
        key: `project:${project.id}`,
        group: 'projects',
        badge: project.selected ? 'Projeto ativo' : 'Projeto',
        title: project.name,
        subtitle: project.description || project.path,
        target: { kind: 'project', projectId: project.id },
      }))
    }
    const tokens = effectiveTokens.value
    return list
      .filter((project) =>
        matchesTokens(`${project.name} ${project.description} ${project.path} ${project.originRepo ?? ''}`, tokens),
      )
      .map((project) => ({
        key: `project:${project.id}`,
        group: 'projects',
        badge: project.selected ? 'Projeto ativo' : 'Projeto',
        title: project.name,
        subtitle: project.description || project.path,
        snippet: highlightMatch(project.description || project.path, tokens),
        target: { kind: 'project', projectId: project.id },
      }))
  }

  function localAgents(): PaletteItem[] {
    if (!hasQuery.value && effectiveChip.value !== 'agents') return []
    const tokens = effectiveTokens.value
    return roster.agents
      .filter((agent) =>
        matchesTokens(`${agent.name} ${agent.id} ${agent.role} ${agent.cli} ${agent.model}`, tokens),
      )
      .map((agent) => ({
        key: `agent:${agent.id}`,
        group: 'agents',
        badge: 'Agente',
        title: agent.name,
        subtitle: `${agent.role} · ${agent.cli}`,
        snippet: highlightMatch(`${agent.role} · ${agent.cli} (${agent.model})`, tokens),
        target: { kind: 'agent', agentId: agent.id },
      }))
  }

  function localMemories(): PaletteItem[] {
    if (!hasQuery.value && effectiveChip.value !== 'memories') return []
    const tokens = effectiveTokens.value
    return MEMORY_FACTS.filter((fact) =>
      matchesTokens(`${fact.text} ${fact.author} ${fact.usage}`, tokens),
    ).map((fact, idx) => ({
      key: `memory-local:${idx}`,
      group: 'memories',
      badge: fact.suggested ? 'Sugestão' : 'Memória',
      title: fact.text,
      subtitle: `${fact.author} · ${fact.usage}`,
      snippet: highlightMatch(fact.text, tokens),
      target: { kind: 'memory', text: fact.text },
    }))
  }

  function mapBackendHit(hit: SearchHit): PaletteItem | null {
    switch (hit.kind) {
      case 'projects': {
        const pid = hit.projectId ?? Number(hit.id)
        if (!Number.isFinite(pid)) return null
        return {
          key: `project:${pid}`,
          group: 'projects',
          badge: projects.selected?.id === pid ? 'Projeto ativo' : 'Projeto',
          title: hit.title,
          subtitle: hit.subtitle,
          snippet: hit.snippet,
          target: { kind: 'project', projectId: pid },
        }
      }
      case 'agents':
        return {
          key: `agent:${hit.id}`,
          group: 'agents',
          badge: 'Agente',
          title: hit.title,
          subtitle: hit.subtitle,
          snippet: hit.snippet,
          target: { kind: 'agent', agentId: hit.id },
        }
      case 'cards': {
        if (hit.projectId === undefined) return null
        return {
          key: `card:${hit.projectId}:${hit.id}`,
          group: 'cards',
          badge: hit.column ?? 'Card',
          title: hit.title,
          subtitle: hit.subtitle,
          snippet: hit.snippet,
          projectName: hit.projectName,
          target: { kind: 'card', projectId: hit.projectId, itemId: hit.itemId ?? hit.id },
        }
      }
      case 'plans': {
        if (hit.projectId === undefined || !hit.itemId) return null
        return {
          key: `plan:${hit.projectId}:${hit.id}`,
          group: 'artifacts',
          badge: ARTIFACT_KIND_LABELS.plano,
          title: hit.title,
          subtitle: hit.subtitle,
          snippet: hit.snippet,
          projectName: hit.projectName,
          target: { kind: 'plan', projectId: hit.projectId, itemId: hit.itemId },
        }
      }
      case 'artifacts': {
        if (hit.projectId === undefined) return null
        return {
          key: `artifact:${hit.projectId}:${hit.id}`,
          group: 'artifacts',
          badge: ARTIFACT_KIND_LABELS[hit.artifactKind ?? ''] ?? 'Artefato',
          title: hit.title,
          subtitle: hit.subtitle,
          snippet: hit.snippet,
          projectName: hit.projectName,
          target: { kind: 'artifact', projectId: hit.projectId, artifactId: hit.id },
        }
      }
      case 'memories':
        return {
          key: `memory:${hit.projectId ?? 0}:${hit.id}`,
          group: 'memories',
          badge: 'Memória',
          title: hit.title,
          subtitle: hit.subtitle,
          snippet: hit.snippet,
          projectName: hit.projectName,
          target: { kind: 'memory', text: hit.title },
        }
      case 'messages': {
        if (hit.projectId === undefined) return null
        const sender = hit.fromAgent ?? 'user'
        const senderLabel = sender === 'user' ? 'Você' : roster.nameOf(sender)
        const receiverLabel = hit.toAgent ? (hit.toAgent === 'user' ? 'Você' : roster.nameOf(hit.toAgent)) : ''
        return {
          key: `message:${hit.projectId}:${hit.id}`,
          group: 'messages',
          badge: 'Mensagem',
          title: hit.title,
          subtitle: [
            receiverLabel ? `${senderLabel} → ${receiverLabel}` : senderLabel,
            hit.itemId,
          ]
            .filter(Boolean)
            .join(' · '),
          snippet: hit.snippet,
          projectName: hit.projectName,
          target: {
            kind: 'message',
            projectId: hit.projectId,
            timestamp: hit.timestamp ?? 0,
            fromAgent: sender,
            text: hit.title,
            channelId: hit.channelId,
            itemId: hit.itemId,
          },
        }
      }
    }
  }

  function mergeUnique(primary: PaletteItem[], secondary: PaletteItem[]): PaletteItem[] {
    const seen = new Set(primary.map((item) => item.key))
    const merged = [...primary]
    for (const item of secondary) {
      if (!seen.has(item.key)) {
        seen.add(item.key)
        merged.push(item)
      }
    }
    return merged
  }

  const groups = computed<PaletteGroup[]>(() => {
    const chip = effectiveChip.value
    const out: PaletteGroup[] = []

    if (!hasQuery.value && chip === 'all' && recents.value.length > 0) {
      out.push({
        id: 'recent',
        label: GROUP_LABELS.recent,
        items: recents.value.map((entry) => ({
          key: `recent:${entry.key}`,
          group: 'recent',
          badge: entry.badge,
          title: entry.title,
          subtitle: entry.subtitle,
          projectName: entry.projectName,
          target: entry.target,
        })),
        total: recents.value.length,
      })
    }

    if (chip === 'all' || chip === 'actions') {
      const items = filterActions()
      if (items.length > 0) {
        out.push({ id: 'actions', label: GROUP_LABELS.actions, items, total: items.length })
      }
    }

    const byKind = new Map<SearchKind, SearchGroup>()
    for (const group of backendGroups.value) byKind.set(group.kind, group)

    if (chip === 'all' || chip === 'projects') {
      const backend = byKind.get('projects')
      const backendItems = (backend?.hits ?? []).map(mapBackendHit).filter((item): item is PaletteItem => item !== null)
      const items = mergeUnique(backendItems, localProjects())
      if (items.length > 0) {
        out.push({
          id: 'projects',
          label: GROUP_LABELS.projects,
          items,
          total: Math.max(items.length, backend?.total ?? 0),
          nextCursor: backend?.nextCursor,
          backendKind: 'projects',
        })
      }
    }

    if (chip === 'all' || chip === 'agents') {
      const backend = byKind.get('agents')
      const backendItems = (backend?.hits ?? []).map(mapBackendHit).filter((item): item is PaletteItem => item !== null)
      const items = mergeUnique(backendItems, localAgents())
      if (items.length > 0) {
        out.push({
          id: 'agents',
          label: GROUP_LABELS.agents,
          items,
          total: Math.max(items.length, backend?.total ?? 0),
          nextCursor: backend?.nextCursor,
          backendKind: 'agents',
        })
      }
    }

    if (chip === 'all' || chip === 'cards') {
      const backend = byKind.get('cards')
      const items = (backend?.hits ?? []).map(mapBackendHit).filter((item): item is PaletteItem => item !== null)
      if (items.length > 0) {
        out.push({
          id: 'cards',
          label: GROUP_LABELS.cards,
          items,
          total: backend?.total ?? items.length,
          nextCursor: backend?.nextCursor,
          backendKind: 'cards',
        })
      }
    }

    if (chip === 'all' || chip === 'artifacts') {
      const plansGroup = byKind.get('plans')
      const docsGroup = byKind.get('artifacts')
      const planItems = (plansGroup?.hits ?? []).map(mapBackendHit).filter((item): item is PaletteItem => item !== null)
      const docItems = (docsGroup?.hits ?? []).map(mapBackendHit).filter((item): item is PaletteItem => item !== null)
      const items = mergeUnique(planItems, docItems)
      if (items.length > 0) {
        out.push({
          id: 'artifacts',
          label: GROUP_LABELS.artifacts,
          items,
          total: (plansGroup?.total ?? 0) + (docsGroup?.total ?? 0),
          nextCursor: docsGroup?.nextCursor ?? plansGroup?.nextCursor,
          backendKind: docsGroup?.nextCursor ? 'artifacts' : 'plans',
        })
      }
    }

    if (chip === 'all' || chip === 'memories') {
      const backend = byKind.get('memories')
      const backendItems = (backend?.hits ?? []).map(mapBackendHit).filter((item): item is PaletteItem => item !== null)
      const items = mergeUnique(backendItems, localMemories())
      if (items.length > 0) {
        out.push({
          id: 'memories',
          label: GROUP_LABELS.memories,
          items,
          total: Math.max(items.length, backend?.total ?? 0),
          nextCursor: backend?.nextCursor,
          backendKind: 'memories',
        })
      }
    }

    if (chip === 'all' || chip === 'messages') {
      const backend = byKind.get('messages')
      const items = (backend?.hits ?? []).map(mapBackendHit).filter((item): item is PaletteItem => item !== null)
      if (items.length > 0) {
        out.push({
          id: 'messages',
          label: GROUP_LABELS.messages,
          items,
          total: backend?.total ?? items.length,
          nextCursor: backend?.nextCursor,
          backendKind: 'messages',
        })
      }
    }

    return out
  })

  function cancelPending(): void {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer)
      debounceTimer = null
    }
  }

  async function runBackendSearch(seq: number): Promise<void> {
    const term = effectiveTerm.value
    const kinds = backendKindsForChip(effectiveChip.value)
    if (!term || kinds.length === 0) {
      backendGroups.value = []
      loading.value = false
      error.value = null
      return
    }
    loading.value = true
    error.value = null
    try {
      const response = await globalSearch({
        query: term,
        projectId: currentProjectOnly.value ? (projects.selected?.id ?? null) : null,
        kinds,
        limit: DEFAULT_GROUP_LIMIT,
      })
      if (seq !== requestSeq) return
      backendGroups.value = response.groups
    } catch (cause) {
      if (seq !== requestSeq) return
      error.value = cause instanceof Error ? cause.message : String(cause)
      backendGroups.value = []
    } finally {
      if (seq === requestSeq) loading.value = false
    }
  }

  function scheduleBackendSearch(): void {
    cancelPending()
    const seq = (requestSeq += 1)
    if (!isOpen.value || !effectiveTerm.value) {
      backendGroups.value = []
      loading.value = false
      error.value = null
      return
    }
    if (backendKindsForChip(effectiveChip.value).length === 0) {
      backendGroups.value = []
      loading.value = false
      return
    }
    debounceTimer = setTimeout(() => {
      debounceTimer = null
      void runBackendSearch(seq)
    }, DEBOUNCE_MS)
  }

  watch([isOpen, query, selectedChip, currentProjectOnly], scheduleBackendSearch)

  async function loadMore(kind: SearchKind): Promise<void> {
    const existing = backendGroups.value.find((group) => group.kind === kind)
    if (!existing?.nextCursor || !effectiveTerm.value) return
    const seq = requestSeq
    try {
      const response = await globalSearch({
        query: effectiveTerm.value,
        projectId: currentProjectOnly.value ? (projects.selected?.id ?? null) : null,
        kinds: [kind],
        limit: DEFAULT_GROUP_LIMIT,
        cursor: existing.nextCursor,
      })
      if (seq !== requestSeq) return
      const incoming = response.groups.find((group) => group.kind === kind)
      if (!incoming) return
      backendGroups.value = backendGroups.value.map((group) =>
        group.kind === kind
          ? {
              ...group,
              hits: [...group.hits, ...incoming.hits],
              nextCursor: incoming.nextCursor,
            }
          : group,
      )
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause)
    }
  }

  function recordRecent(item: PaletteItem): void {
    const cleanKey = item.key.replace(/^recent:/, '')
    const entry: RecentSearchEntry = {
      key: cleanKey,
      badge: item.badge,
      title: item.title,
      subtitle: item.subtitle,
      projectName: item.projectName,
      target: item.target,
    }
    recents.value = [entry, ...recents.value.filter((existing) => existing.key !== cleanKey)].slice(0, MAX_RECENTS)
    writeRecents(recents.value)
  }

  function openDialog(): void {
    isOpen.value = true
    error.value = null
    if (roster.agents.length === 0) void roster.load().catch(() => {})
    if (!projects.loaded) void projects.load().catch(() => {})
  }

  function closeDialog(): void {
    cancelPending()
    requestSeq += 1
    isOpen.value = false
    query.value = ''
    selectedChip.value = 'all'
    backendGroups.value = []
    loading.value = false
    error.value = null
  }

  function toggle(): void {
    if (isOpen.value) closeDialog()
    else openDialog()
  }

  function selectChip(chip: SearchFilterChip): void {
    if (parsed.value.prefixChip !== null) {
      query.value = parsed.value.term
    }
    selectedChip.value = chip
  }

  async function ensureProjectSelected(projectId: number): Promise<void> {
    if (projects.selected?.id === projectId) return
    await projects.select(projectId)
  }

  async function execute(item: PaletteItem): Promise<void> {
    recordRecent(item)
    const target = item.target
    closeDialog()

    switch (target.kind) {
      case 'action':
        if (target.actionId === 'new-request') backlog.startRequest()
        else if (target.actionId === 'toggle-rail') rail.toggle()
        return
      case 'page':
        navigation.goTo(target.page)
        return
      case 'settings':
        navigation.openSettingsSection(target.section)
        return
      case 'providers':
        navigation.goTo('config')
        providers.openSetup(target.providerId)
        return
      case 'project': {
        const found = projects.projects.find((project) => project.id === target.projectId)
        if (found) await navigation.enterProject(found)
        return
      }
      case 'agent':
        if (projects.selected !== null) {
          feed.showScreen({ kind: 'agent', agent: target.agentId })
          navigation.goTo('conversas')
        } else {
          navigation.goTo('agentes')
        }
        return
      case 'card':
        await ensureProjectSelected(target.projectId)
        navigation.openCard(target.itemId)
        return
      case 'plan':
        await ensureProjectSelected(target.projectId)
        navigation.openPlanArtifact(target.itemId)
        return
      case 'artifact':
        await ensureProjectSelected(target.projectId)
        navigation.openArtifactDocument(target.artifactId)
        return
      case 'memory':
        navigation.goTo('memoria')
        return
      case 'message':
        await ensureProjectSelected(target.projectId)
        navigation.openMessage({
          time: target.timestamp,
          from: target.fromAgent,
          text: target.text,
          channelId: target.channelId,
          itemId: target.itemId,
        })
        return
    }
  }

  return {
    isOpen,
    query,
    selectedChip,
    effectiveChip,
    effectiveTerm,
    currentProjectOnly,
    loading,
    error,
    hasQuery,
    groups,
    openDialog,
    closeDialog,
    toggle,
    selectChip,
    loadMore,
    execute,
  }
})
