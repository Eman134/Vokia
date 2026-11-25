import type {
  ProviderPlanUsage,
  ProviderUsageWindow,
  UsageLimitEntry,
} from '../team/events'
import type {
  ModelGroupingMode,
  ModelTier,
  ModelTiers,
  ReasoningEffort,
  ReasoningEffortSpec,
} from '../team/teamApi'
import {
  MAX_PERCENT,
  MS_PER_SECOND,
  clampPercent,
  isUsingCredits,
  isWindowExpired,
  mostRestrictiveWindow,
  toneOf,
  type UsageTone,
} from '../usage/usagePlans'
import type { ProviderConnectionStatus, ProviderWithStatus } from './providersApi'

export type ProviderRailStatus =
  | 'checking'
  | 'connected'
  | 'signed-out'
  | 'missing'
  | 'limit-reached'
  | 'error'

export interface EffortOption {
  value: ReasoningEffort
  label: string
}

export const REASONING_EFFORT_OPTIONS: readonly EffortOption[] = [
  { value: 'low', label: 'Baixo' },
  { value: 'medium', label: 'Médio' },
  { value: 'high', label: 'Alto' },
  { value: 'max', label: 'Máximo' },
]

export const EFFORT_LABELS: Record<ReasoningEffort, string> = {
  low: 'Baixo',
  medium: 'Médio',
  high: 'Alto',
  max: 'Máximo',
}

export const RAIL_STATUS_LABELS: Record<ProviderRailStatus, string> = {
  checking: 'Verificando',
  connected: 'Conectado',
  'signed-out': 'Sem login',
  missing: 'Ausente',
  'limit-reached': 'Limite atingido',
  error: 'Erro',
}

const TIER_BADGE_LABELS: Record<ModelTier, string> = {
  basic: 'Básico',
  intermediate: 'Intermediário',
  advanced: 'Avançado',
}

const CANONICAL_EFFORT_ORDER: readonly ReasoningEffort[] = ['low', 'medium', 'high', 'max']

const KNOWN_SERVICE_LABELS: Record<string, string> = {
  openai: 'OpenAI',
  anthropic: 'Anthropic',
  google: 'Google',
  deepseek: 'DeepSeek',
  xai: 'xAI',
  meta: 'Meta',
  mistral: 'Mistral',
  qwen: 'Qwen',
  zai: 'Z.AI',
  moonshot: 'Moonshot',
  openrouter: 'OpenRouter',
}

export interface ProviderCatalogSource {
  id: string
  label: string
  installed: boolean
  connectionStatus?: ProviderConnectionStatus
  statusDetail?: string | null
  models: readonly string[]
  freeModels?: readonly string[]
  defaultModel: string
  tiers: ModelTiers
  modelGrouping?: ModelGroupingMode
  effort?: ReasoningEffortSpec | null
  supportsFreeWithoutLogin?: boolean
  installHint?: string | null
  connectHint?: string | null
  badgeCode?: string | null
  displayOrder?: number | null
}

export interface ProviderQuotaSummary {
  usedPercent: number
  remainingPercent: number
  usingCredits: boolean
  limitedUntilLabel: string | null
  tone: UsageTone | 'accent'
}

export interface ProviderRailItem {
  id: string
  label: string
  badgeCode: string
  installed: boolean
  status: ProviderRailStatus
  statusLabel: string
  statusDetail: string | null
  quota: ProviderQuotaSummary | null
  installHint: string | null
  connectHint: string | null
  supportsFreeWithoutLogin: boolean
  isActive: boolean
}

export interface ModelLimitBadge {
  kind: 'limit' | 'credits'
  label: string
}

export interface CollapsedModelItem {
  providerId: string
  providerLabel: string
  baseModelId: string
  displayName: string
  installed: boolean
  selectable: boolean
  isFree: boolean
  tierLabels: string[]
  supportedEfforts: ReasoningEffort[]
  defaultEffort: ReasoningEffort | null
  variantModelByEffort: Partial<Record<ReasoningEffort, string>>
  subGroup: string | null
  limitBadge: ModelLimitBadge | null
  isActive: boolean
}

export interface CatalogModelSection {
  id: string
  label: string | null
  isFreeSection: boolean
  models: CollapsedModelItem[]
}

export function normalizeReasoningEffort(raw?: string | null): ReasoningEffort | null {
  if (!raw) return null
  const lower = raw.trim().toLowerCase()
  if (lower === 'low' || lower === 'medium' || lower === 'high' || lower === 'max') {
    return lower
  }
  if (lower === 'xhigh' || lower === 'maximum') {
    return 'max'
  }
  return null
}

export function isFreeModel(modelId: string, explicitFreeModels?: readonly string[]): boolean {
  if (explicitFreeModels?.includes(modelId)) return true
  const lower = modelId.toLowerCase()
  return (
    lower.endsWith(':free') ||
    lower.endsWith('-free') ||
    lower.endsWith('/free') ||
    /\bfree\b/.test(lower)
  )
}

export function formatLimitTimeHHMM(resetsAtSecs: number): string | null {
  if (!Number.isFinite(resetsAtSecs) || resetsAtSecs <= 0) return null
  const date = new Date(resetsAtSecs * MS_PER_SECOND)
  if (Number.isNaN(date.getTime())) return null
  const hours = String(date.getHours()).padStart(2, '0')
  const minutes = String(date.getMinutes()).padStart(2, '0')
  return `${hours}:${minutes}`
}

export function stripModelEffortSuffix(
  modelId: string,
  spec?: ReasoningEffortSpec | null,
): { baseModelId: string; effort: ReasoningEffort | null; separator: string | null } {
  const separators: string[] = []
  if (spec?.suffixSeparator) {
    separators.push(spec.suffixSeparator)
  }
  for (const fallbackSep of ['/', '-']) {
    if (!separators.includes(fallbackSep)) {
      separators.push(fallbackSep)
    }
  }

  for (const sep of separators) {
    const idx = modelId.lastIndexOf(sep)
    if (idx <= 0 || idx === modelId.length - 1) continue
    const suffix = modelId.slice(idx + sep.length)
    const normalized = normalizeReasoningEffort(suffix)
    if (!normalized) continue
    const base = modelId.slice(0, idx)
    if (!base) continue
    return { baseModelId: base, effort: normalized, separator: sep }
  }

  return { baseModelId: modelId, effort: null, separator: null }
}

export function deriveBadgeCode(provider: Pick<ProviderCatalogSource, 'label' | 'id' | 'badgeCode'>): string {
  const explicit = provider.badgeCode?.trim()
  if (explicit) return explicit.slice(0, 2).toUpperCase()
  const words = provider.label.trim().split(/\s+/).filter(Boolean)
  if (words.length >= 2) {
    return `${words[0][0]}${words[1][0]}`.toUpperCase()
  }
  const source = words[0] ?? provider.id
  return source.slice(0, 2).toUpperCase()
}

export function providerSourceFromStatus(provider: ProviderWithStatus): ProviderCatalogSource {
  return {
    id: provider.id,
    label: provider.label,
    installed: provider.installed,
    connectionStatus: provider.connectionStatus,
    statusDetail: provider.statusDetail ?? null,
    models: provider.models,
    freeModels: provider.freeModels ?? [],
    defaultModel: provider.defaultModel,
    tiers: provider.tiers,
    modelGrouping: provider.modelGrouping,
    effort: provider.effort ?? null,
    supportsFreeWithoutLogin: Boolean(provider.authProbe?.freeWithoutLogin),
    installHint: provider.installHint ?? null,
    connectHint: provider.connectHint ?? null,
    badgeCode: provider.badgeCode ?? null,
    displayOrder: provider.displayOrder ?? null,
  }
}

function isWindowExhausted(window: ProviderUsageWindow, nowMs: number): boolean {
  if (isWindowExpired(window, nowMs)) return false
  return window.percent >= MAX_PERCENT || Boolean(window.rejected)
}

function findProviderQuotaSummary(
  providerId: string,
  usage: ProviderPlanUsage | undefined,
  activeLimits: readonly UsageLimitEntry[],
  nowMs: number,
): ProviderQuotaSummary | null {
  const providerLimit = activeLimits.find(
    (entry) => entry.providerId === providerId && entry.releaseAtMs > nowMs,
  )
  if (!usage && !providerLimit) return null

  const usingCredits = usage ? isUsingCredits(usage) : false
  const worst = usage ? mostRestrictiveWindow(usage, nowMs) : null
  const usedPercent = worst ? clampPercent(worst.percent) : providerLimit ? MAX_PERCENT : 0
  const remainingPercent = Math.max(0, MAX_PERCENT - usedPercent)

  let limitedUntilLabel: string | null = null
  if (!usingCredits) {
    const exhaustedWindow = worst && isWindowExhausted(worst, nowMs) ? worst : null
    if (exhaustedWindow) {
      const hhmm = formatLimitTimeHHMM(exhaustedWindow.resetsAt)
      limitedUntilLabel = hhmm ? `Limite até ${hhmm}` : 'Limite atingido'
    } else if (providerLimit) {
      const hhmm = formatLimitTimeHHMM(
        providerLimit.resetsAt ?? Math.floor(providerLimit.releaseAtMs / MS_PER_SECOND),
      )
      limitedUntilLabel = hhmm ? `Limite até ${hhmm}` : 'Limite atingido'
    }
  }

  const tone: UsageTone | 'accent' = usingCredits
    ? 'accent'
    : limitedUntilLabel
      ? 'danger'
      : toneOf(usedPercent)

  return {
    usedPercent,
    remainingPercent,
    usingCredits,
    limitedUntilLabel,
    tone,
  }
}

export function buildProviderRailItems(params: {
  providers: readonly ProviderCatalogSource[]
  usageByProvider: Record<string, ProviderPlanUsage>
  activeLimits?: readonly UsageLimitEntry[]
  discovering: Record<string, boolean>
  checkingStatus: Record<string, boolean>
  activeProviderId: string | null | undefined
  nowMs: number
}): ProviderRailItem[] {
  const {
    providers,
    usageByProvider,
    activeLimits = [],
    discovering,
    checkingStatus,
    activeProviderId,
    nowMs,
  } = params

  const sorted = [...providers].sort((a, b) => {
    const orderA = a.displayOrder ?? 1000
    const orderB = b.displayOrder ?? 1000
    if (orderA !== orderB) return orderA - orderB
    return a.label.localeCompare(b.label)
  })

  return sorted.map((provider) => {
    const usage = usageByProvider[provider.id]
    const quota = findProviderQuotaSummary(provider.id, usage, activeLimits, nowMs)
    const isChecking = Boolean(checkingStatus[provider.id] || discovering[provider.id])

    let status: ProviderRailStatus
    if (isChecking) {
      status = 'checking'
    } else if (!provider.installed || provider.connectionStatus === 'missing') {
      status = 'missing'
    } else if (provider.connectionStatus === 'error') {
      status = 'error'
    } else if (provider.connectionStatus === 'signed-out' && !provider.supportsFreeWithoutLogin) {
      status = 'signed-out'
    } else if (quota?.limitedUntilLabel && !quota.usingCredits) {
      status = 'limit-reached'
    } else {
      status = 'connected'
    }

    return {
      id: provider.id,
      label: provider.label,
      badgeCode: deriveBadgeCode(provider),
      installed: provider.installed,
      status,
      statusLabel: RAIL_STATUS_LABELS[status],
      statusDetail: provider.statusDetail ?? null,
      quota,
      installHint: provider.installHint ?? null,
      connectHint: provider.connectHint ?? null,
      supportsFreeWithoutLogin: Boolean(provider.supportsFreeWithoutLogin),
      isActive: Boolean(activeProviderId && activeProviderId === provider.id),
    }
  })
}

function collectRawModels(
  provider: ProviderCatalogSource,
  currentActiveModel: string | null,
): string[] {
  const seen = new Set<string>()
  const push = (val: string | null | undefined) => {
    const trimmed = val?.trim()
    if (trimmed && !seen.has(trimmed)) {
      seen.add(trimmed)
    }
  }
  for (const m of provider.models) push(m)
  push(provider.tiers.advanced)
  push(provider.tiers.intermediate)
  push(provider.tiers.basic)
  push(provider.defaultModel)
  push(currentActiveModel)
  return Array.from(seen)
}

function ruleEffortsForBaseModel(
  baseModelId: string,
  spec: ReasoningEffortSpec | null | undefined,
): ReasoningEffort[] {
  if (!spec) return []
  const lower = baseModelId.toLowerCase()
  for (const rule of spec.modelRules ?? []) {
    const pat = rule.pattern.trim().toLowerCase()
    if (pat && lower.includes(pat)) {
      return orderCanonicalEfforts(rule.levels)
    }
  }
  if (spec.applyToAllModels) {
    return orderCanonicalEfforts(spec.levels)
  }
  return []
}

function orderCanonicalEfforts(levels: readonly (string | null | undefined)[]): ReasoningEffort[] {
  const found = new Set<ReasoningEffort>()
  for (const raw of levels) {
    const normalized = normalizeReasoningEffort(raw)
    if (normalized) found.add(normalized)
  }
  return CANONICAL_EFFORT_ORDER.filter((lvl) => found.has(lvl))
}

function resolveModelLimitBadge(params: {
  providerId: string
  baseModelId: string
  rawVariants: readonly string[]
  usage: ProviderPlanUsage | undefined
  activeLimits: readonly UsageLimitEntry[]
  nowMs: number
}): ModelLimitBadge | null {
  const { providerId, baseModelId, rawVariants, usage, activeLimits, nowMs } = params
  const candidates = new Set([baseModelId.toLowerCase(), ...rawVariants.map((m) => m.toLowerCase())])

  // 1. Verifica grupos específicos por modelo na leitura de cota (ex.: Gemini por grupo de modelo)
  const matchingGroup = usage?.groups?.find((group) =>
    group.models?.some((m) => {
      const lower = m.toLowerCase()
      if (candidates.has(lower)) return true
      return Array.from(candidates).some((c) => c.includes(lower) || lower.includes(c))
    }),
  )
  if (matchingGroup && isWindowExhausted(matchingGroup.window, nowMs)) {
    if (usage?.creditsAvailable === true) {
      return { kind: 'credits', label: 'Usando créditos' }
    }
    const hhmm = formatLimitTimeHHMM(matchingGroup.window.resetsAt)
    return { kind: 'limit', label: hhmm ? `Limite até ${hhmm}` : 'Limite atingido' }
  }

  // 2. Verifica limites ativos registrados por (providerId, model)
  const matchingLimit = activeLimits.find(
    (entry) =>
      entry.providerId === providerId &&
      entry.releaseAtMs > nowMs &&
      (candidates.has(entry.model.toLowerCase()) ||
        stripModelEffortSuffix(entry.model).baseModelId.toLowerCase() === baseModelId.toLowerCase()),
  )
  if (matchingLimit) {
    if (usage?.creditsAvailable === true) {
      return { kind: 'credits', label: 'Usando créditos' }
    }
    const hhmm = formatLimitTimeHHMM(
      matchingLimit.resetsAt ?? Math.floor(matchingLimit.releaseAtMs / MS_PER_SECOND),
    )
    return { kind: 'limit', label: hhmm ? `Limite até ${hhmm}` : 'Limite atingido' }
  }

  // 3. Verifica janelas globais do provedor (5h / semanal / terciária)
  if (usage) {
    const providerWindows = [usage.primary, usage.secondary, usage.tertiary].filter(
      (w): w is ProviderUsageWindow => Boolean(w),
    )
    const exhausted = providerWindows.find((w) => isWindowExhausted(w, nowMs))
    if (exhausted) {
      if (usage.creditsAvailable === true) {
        return { kind: 'credits', label: 'Usando créditos' }
      }
      const hhmm = formatLimitTimeHHMM(exhausted.resetsAt)
      return { kind: 'limit', label: hhmm ? `Limite até ${hhmm}` : 'Limite atingido' }
    }
  }

  return null
}

function formatServiceGroupLabel(prefix: string, provider: ProviderCatalogSource): string {
  const lower = prefix.trim().toLowerCase()
  if (lower === provider.id.toLowerCase()) {
    return `${provider.label} Go`
  }
  if (KNOWN_SERVICE_LABELS[lower]) {
    return KNOWN_SERVICE_LABELS[lower]
  }
  return lower.charAt(0).toUpperCase() + lower.slice(1)
}

function tierLabelsForVariants(variants: readonly string[], tiers: ModelTiers): string[] {
  const set = new Set(variants)
  const labels: string[] = []
  if (tiers.basic?.trim() && set.has(tiers.basic.trim())) labels.push(TIER_BADGE_LABELS.basic)
  if (tiers.intermediate?.trim() && set.has(tiers.intermediate.trim())) {
    labels.push(TIER_BADGE_LABELS.intermediate)
  }
  if (tiers.advanced?.trim() && set.has(tiers.advanced.trim())) {
    labels.push(TIER_BADGE_LABELS.advanced)
  }
  return labels
}

export function buildCollapsedModelSections(params: {
  provider: ProviderCatalogSource
  usage?: ProviderPlanUsage
  activeLimits?: readonly UsageLimitEntry[]
  activeProviderId: string | null | undefined
  activeModelId: string | null | undefined
  searchQuery: string
  nowMs: number
}): CatalogModelSection[] {
  const {
    provider,
    usage,
    activeLimits = [],
    activeProviderId,
    activeModelId,
    searchQuery,
    nowMs,
  } = params

  const isActiveProvider = Boolean(activeProviderId && activeProviderId === provider.id)
  const activeBase = isActiveProvider && activeModelId
    ? stripModelEffortSuffix(activeModelId, provider.effort).baseModelId
    : null

  const rawModels = collectRawModels(
    provider,
    isActiveProvider ? (activeModelId ?? null) : null,
  )

  interface Bucket {
    baseModelId: string
    rawVariants: string[]
    variantModelByEffort: Partial<Record<ReasoningEffort, string>>
    discoveredEfforts: ReasoningEffort[]
    isFree: boolean
  }

  const buckets = new Map<string, Bucket>()

  for (const rawModel of rawModels) {
    const parsed = stripModelEffortSuffix(rawModel, provider.effort)
    const shouldCollapse =
      parsed.effort !== null &&
      (provider.effort?.mode === 'model-suffix' ||
        rawModels.some(
          (other) =>
            other !== rawModel &&
            stripModelEffortSuffix(other, provider.effort).baseModelId === parsed.baseModelId,
        ))
    const baseKey = shouldCollapse ? parsed.baseModelId : rawModel
    let bucket = buckets.get(baseKey)
    if (!bucket) {
      bucket = {
        baseModelId: baseKey,
        rawVariants: [],
        variantModelByEffort: {},
        discoveredEfforts: [],
        isFree: false,
      }
      buckets.set(baseKey, bucket)
    }
    bucket.rawVariants.push(rawModel)
    if (shouldCollapse && parsed.effort) {
      bucket.variantModelByEffort[parsed.effort] = rawModel
      if (!bucket.discoveredEfforts.includes(parsed.effort)) {
        bucket.discoveredEfforts.push(parsed.effort)
      }
    }
    if (isFreeModel(rawModel, provider.freeModels)) {
      bucket.isFree = true
    }
  }

  const query = searchQuery.trim().toLowerCase()
  const usePrefixGrouping = provider.modelGrouping === 'by-prefix-free-first'
  const items: CollapsedModelItem[] = []

  for (const bucket of buckets.values()) {
    const ruleEfforts = ruleEffortsForBaseModel(bucket.baseModelId, provider.effort)
    const supportedEfforts = orderCanonicalEfforts([
      ...ruleEfforts,
      ...bucket.discoveredEfforts,
    ])
    const specDefault = normalizeReasoningEffort(provider.effort?.defaultLevel)
    const defaultEffort: ReasoningEffort | null =
      supportedEfforts.length === 0
        ? null
        : specDefault && supportedEfforts.includes(specDefault)
          ? specDefault
          : supportedEfforts.includes('medium')
            ? 'medium'
            : supportedEfforts[0]

    let subGroup: string | null = null
    if (usePrefixGrouping) {
      if (bucket.isFree) {
        subGroup = `${provider.label} · Grátis`
      } else {
        const slashIdx = bucket.baseModelId.indexOf('/')
        if (slashIdx > 0) {
          subGroup = formatServiceGroupLabel(bucket.baseModelId.slice(0, slashIdx), provider)
        } else {
          subGroup = 'Outros modelos'
        }
      }
    }

    const displayName =
      usePrefixGrouping && bucket.baseModelId.includes('/')
        ? bucket.baseModelId.slice(bucket.baseModelId.indexOf('/') + 1)
        : bucket.baseModelId

    if (query) {
      const haystack = `${bucket.baseModelId} ${displayName} ${subGroup ?? ''} ${provider.label}`.toLowerCase()
      if (!haystack.includes(query)) continue
    }

    const limitBadge = resolveModelLimitBadge({
      providerId: provider.id,
      baseModelId: bucket.baseModelId,
      rawVariants: bucket.rawVariants,
      usage,
      activeLimits,
      nowMs,
    })

    const isActive =
      isActiveProvider &&
      (activeModelId === bucket.baseModelId ||
        bucket.rawVariants.includes(activeModelId ?? '') ||
        (activeBase !== null && activeBase === bucket.baseModelId))

    items.push({
      providerId: provider.id,
      providerLabel: provider.label,
      baseModelId: bucket.baseModelId,
      displayName,
      installed: provider.installed,
      selectable: provider.installed,
      isFree: bucket.isFree,
      tierLabels: tierLabelsForVariants(bucket.rawVariants, provider.tiers),
      supportedEfforts,
      defaultEffort,
      variantModelByEffort: bucket.variantModelByEffort,
      subGroup,
      limitBadge,
      isActive,
    })
  }

  if (!usePrefixGrouping) {
    if (items.length === 0) return []
    const sorted = [...items].sort((a, b) => {
      if (a.isFree !== b.isFree) return a.isFree ? -1 : 1
      return 0
    })
    return [
      {
        id: `${provider.id}:all`,
        label: null,
        isFreeSection: false,
        models: sorted,
      },
    ]
  }

  // Agrupamento by-prefix-free-first: Grátis sempre primeiro, seguido por serviço (Go antes dos demais)
  const freeLabel = `${provider.label} · Grátis`
  const goLabel = `${provider.label} Go`
  const sectionMap = new Map<string, CollapsedModelItem[]>()

  for (const item of items) {
    const groupLabel = item.subGroup ?? 'Outros modelos'
    const existing = sectionMap.get(groupLabel)
    if (existing) {
      existing.push(item)
    } else {
      sectionMap.set(groupLabel, [item])
    }
  }

  const orderedLabels = Array.from(sectionMap.keys()).sort((a, b) => {
    if (a === freeLabel) return -1
    if (b === freeLabel) return 1
    if (a === goLabel) return -1
    if (b === goLabel) return 1
    if (a === 'Outros modelos') return 1
    if (b === 'Outros modelos') return -1
    return a.localeCompare(b)
  })

  return orderedLabels.map((label) => ({
    id: `${provider.id}:${label}`,
    label,
    isFreeSection: label === freeLabel,
    models: sectionMap.get(label) ?? [],
  }))
}

export function resolveModelSelectionPayload(
  provider: ProviderCatalogSource,
  item: CollapsedModelItem,
  chosenEffort: ReasoningEffort | null,
): {
  cli: string
  model: string
  modelTier: null
  reasoningEffort: ReasoningEffort | null
} {
  const effectiveEffort: ReasoningEffort | null =
    item.supportedEfforts.length > 0
      ? chosenEffort && item.supportedEfforts.includes(chosenEffort)
        ? chosenEffort
        : item.defaultEffort
      : null

  let resolvedModel = item.baseModelId
  if (effectiveEffort) {
    const discoveredVariant = item.variantModelByEffort[effectiveEffort]
    if (discoveredVariant) {
      resolvedModel = discoveredVariant
    } else if (provider.effort?.mode === 'model-suffix') {
      const sep = provider.effort.suffixSeparator ?? '-'
      const mapped = provider.effort.valueMap?.[effectiveEffort] ?? effectiveEffort
      resolvedModel = `${item.baseModelId}${sep}${mapped}`
    }
  }

  return {
    cli: provider.id,
    model: resolvedModel,
    modelTier: null,
    reasoningEffort: effectiveEffort,
  }
}
