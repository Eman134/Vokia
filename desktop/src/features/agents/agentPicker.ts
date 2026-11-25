import type { Agent, AgentState, TemporaryOverride } from '../team/events'
import type { AgentProfile, CliPreset, ModelTier } from '../team/teamApi'
import type { ActiveLlmConfig } from '../providers/useActiveLlm'

export const ALL_AGENTS_OPTION_ID = '__all_agents__'
export const DEFAULT_ALL_AGENTS_LABEL = 'Todos os agentes'

export type AgentPresenceKind = 'working' | 'waiting' | 'paused' | 'idle'

export interface AgentPresenceMeta {
  kind: AgentPresenceKind
  label: string
  shortLabel: string
  badgeClasses: string
  priority: number
}

export interface ResolvedAgentLlm {
  providerId: string
  providerLabel: string
  model: string
  shortModel: string
  chipLabel: string
  isOverride: boolean
}

export interface AgentPickerAllOption {
  kind: 'all'
  id: typeof ALL_AGENTS_OPTION_ID
  label: string
  count: number
}

export interface AgentPickerAgentOption {
  kind: 'agent'
  id: Agent
  label: string
  agent: AgentProfile
  presence: AgentPresenceMeta
  llm: ResolvedAgentLlm
  count: number
}

export type AgentPickerOption = AgentPickerAllOption | AgentPickerAgentOption

export interface BuildAgentPickerOptionsInput {
  agents: readonly AgentProfile[]
  query: string
  states: Readonly<Record<string, AgentState | undefined>>
  isPaused: (agent: Agent) => boolean
  counts?: Readonly<Record<string, number | undefined>>
  presets?: readonly CliPreset[]
  activeLlm?: ActiveLlmConfig | null
  overrides?: readonly TemporaryOverride[]
  busyFirst?: boolean
  allowAll?: boolean
  allLabel?: string
}

const KNOWN_PROVIDER_LABELS: Readonly<Record<string, string>> = {
  claude: 'Claude',
  codex: 'Codex',
  opencode: 'OpenCode',
  antigravity: 'Antigravity',
  gemini: 'Gemini',
}

const PRESENCE_META: Record<AgentPresenceKind, Omit<AgentPresenceMeta, 'shortLabel'>> = {
  working: {
    kind: 'working',
    label: 'Trabalhando',
    badgeClasses: 'bg-ok text-accent-ink border-surface',
    priority: 0,
  },
  waiting: {
    kind: 'waiting',
    label: 'Aguardando',
    badgeClasses: 'bg-warn text-accent-ink border-surface',
    priority: 1,
  },
  idle: {
    kind: 'idle',
    label: 'Livre',
    badgeClasses: 'bg-surface text-muted border-graphite',
    priority: 2,
  },
  paused: {
    kind: 'paused',
    label: 'Pausado',
    badgeClasses: 'bg-surface-2 text-ink border-line',
    priority: 3,
  },
}

export function normalizeSearchText(text: string): string {
  return text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase().trim()
}

export function presenceMetaOf(state: AgentState | undefined, paused: boolean): AgentPresenceMeta {
  if (paused) {
    return { ...PRESENCE_META.paused, shortLabel: 'pausado' }
  }
  if (state === 'thinking') {
    return { ...PRESENCE_META.working, shortLabel: 'trabalhando' }
  }
  if (state === 'sending') {
    return { ...PRESENCE_META.working, shortLabel: 'enviando' }
  }
  if (state === 'waiting') {
    return { ...PRESENCE_META.waiting, shortLabel: 'aguardando' }
  }
  return { ...PRESENCE_META.idle, shortLabel: 'livre' }
}

function capitalize(word: string): string {
  if (!word) return ''
  return `${word.charAt(0).toUpperCase()}${word.slice(1)}`
}

export function formatProviderLabel(providerId: string, presets: readonly CliPreset[] = []): string {
  const normalized = providerId.trim().toLowerCase()
  const preset = presets.find((entry) => entry.id.toLowerCase() === normalized)
  if (preset?.label) return preset.label
  if (KNOWN_PROVIDER_LABELS[normalized]) return KNOWN_PROVIDER_LABELS[normalized]
  return providerId.trim() ? capitalize(providerId.trim()) : ''
}

export function formatShortModel(rawModel: string): string {
  const trimmed = rawModel.trim()
  if (!trimmed) return ''

  const withoutPrefix = trimmed.includes('/') ? trimmed.slice(trimmed.lastIndexOf('/') + 1) : trimmed
  const withoutDate = withoutPrefix.replace(/-\d{8}$/, '').replace(/-\d{4}-\d{2}-\d{2}$/, '')

  const claudeNew = /^claude-(sonnet|opus|haiku)(?:[-.](\d+)(?:[-.](\d+))?)?$/i.exec(withoutDate)
  if (claudeNew) {
    const family = capitalize(claudeNew[1].toLowerCase())
    const major = claudeNew[2]
    const minor = claudeNew[3]
    if (!major) return family
    return minor ? `${family} ${major}.${minor}` : `${family} ${major}`
  }

  const claudeLegacy = /^claude-(\d+)(?:[-.](\d+))?-(sonnet|opus|haiku)$/i.exec(withoutDate)
  if (claudeLegacy) {
    const family = capitalize(claudeLegacy[3].toLowerCase())
    const version = claudeLegacy[2] ? `${claudeLegacy[1]}.${claudeLegacy[2]}` : claudeLegacy[1]
    return `${family} ${version}`
  }

  if (/^gpt-/i.test(withoutDate)) {
    return `GPT-${withoutDate.slice(4)}`
  }

  return withoutDate
}

function isModelTier(value: string | null | undefined): value is ModelTier {
  return value === 'basic' || value === 'intermediate' || value === 'advanced'
}

function resolveModelFromTier(
  preset: CliPreset | undefined,
  tier: string | null | undefined,
): string {
  if (!preset || !isModelTier(tier)) return ''
  return preset.tiers[tier]?.trim() ?? ''
}

export function resolveAgentLlm(
  agent: AgentProfile,
  options: {
    presets?: readonly CliPreset[]
    activeLlm?: ActiveLlmConfig | null
    override?: TemporaryOverride | null
  } = {},
): ResolvedAgentLlm {
  const presets = options.presets ?? []
  const override = options.override ?? null
  const activeLlm = options.activeLlm ?? null

  let providerId: string
  let model: string
  let isOverride = false

  if (override) {
    isOverride = true
    providerId = override.cli
    const preset = presets.find((entry) => entry.id === providerId)
    model =
      override.model?.trim() ||
      resolveModelFromTier(preset, override.modelTier) ||
      preset?.defaultModel?.trim() ||
      ''
  } else if (agent.followDefaultLlm && activeLlm?.cli) {
    providerId = activeLlm.cli
    const preset = presets.find((entry) => entry.id === providerId)
    model =
      activeLlm.model?.trim() ||
      resolveModelFromTier(preset, activeLlm.modelTier) ||
      preset?.defaultModel?.trim() ||
      agent.model.trim()
  } else {
    providerId = agent.cli
    const preset = presets.find((entry) => entry.id === providerId)
    model =
      agent.model.trim() ||
      resolveModelFromTier(preset, agent.modelTier) ||
      preset?.defaultModel?.trim() ||
      ''
  }

  const providerLabel = formatProviderLabel(providerId, presets)
  const shortModel = formatShortModel(model)
  const chipLabel =
    providerLabel && shortModel
      ? `${providerLabel} · ${shortModel}`
      : providerLabel || shortModel

  return {
    providerId,
    providerLabel,
    model,
    shortModel,
    chipLabel,
    isOverride,
  }
}

export function matchesAgentQuery(
  agent: AgentProfile,
  llm: ResolvedAgentLlm,
  presence: AgentPresenceMeta,
  rawQuery: string,
): boolean {
  const needle = normalizeSearchText(rawQuery)
  if (!needle) return true
  const searchable = normalizeSearchText(
    [
      agent.name,
      agent.id,
      agent.role,
      llm.providerId,
      llm.providerLabel,
      llm.model,
      llm.shortModel,
      llm.chipLabel,
      presence.label,
      presence.shortLabel,
    ].join(' '),
  )
  return searchable.includes(needle)
}

export function buildAgentPickerOptions(input: BuildAgentPickerOptionsInput): AgentPickerOption[] {
  const presets = input.presets ?? []
  const overrides = input.overrides ?? []
  const now = Date.now()
  const overrideByAgent = new Map<string, TemporaryOverride>()
  for (const ov of overrides) {
    if (ov.expiresAt == null || ov.expiresAt > now) {
      overrideByAgent.set(ov.agent, ov)
    }
  }

  const mapped: Array<{ option: AgentPickerAgentOption; originalIndex: number }> = []

  input.agents.forEach((agent, index) => {
    const presence = presenceMetaOf(input.states[agent.id], input.isPaused(agent.id))
    const llm = resolveAgentLlm(agent, {
      presets,
      activeLlm: input.activeLlm,
      override: overrideByAgent.get(agent.id) ?? null,
    })
    if (!matchesAgentQuery(agent, llm, presence, input.query)) return

    const count = Math.max(0, Math.floor(input.counts?.[agent.id] ?? 0))
    mapped.push({
      originalIndex: index,
      option: {
        kind: 'agent',
        id: agent.id,
        label: agent.name,
        agent,
        presence,
        llm,
        count,
      },
    })
  })

  if (input.busyFirst) {
    mapped.sort((a, b) => {
      const priorityDiff = a.option.presence.priority - b.option.presence.priority
      if (priorityDiff !== 0) return priorityDiff
      return a.originalIndex - b.originalIndex
    })
  }

  const options: AgentPickerOption[] = mapped.map((entry) => entry.option)

  if (input.allowAll) {
    const allLabel = input.allLabel?.trim() || DEFAULT_ALL_AGENTS_LABEL
    const needle = normalizeSearchText(input.query)
    if (!needle || normalizeSearchText(allLabel).includes(needle)) {
      const totalCount = Object.values(input.counts ?? {}).reduce<number>(
        (sum, value) => sum + Math.max(0, Math.floor(value ?? 0)),
        0,
      )
      options.unshift({
        kind: 'all',
        id: ALL_AGENTS_OPTION_ID,
        label: allLabel,
        count: totalCount,
      })
    }
  }

  return options
}

export function emptyAgentSearchMessage(query: string): string {
  return `Nenhum agente encontrado para «${query.trim()}»`
}
