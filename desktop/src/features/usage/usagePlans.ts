import type {
  PaceStatus,
  ProviderPlanUsage,
  ProviderUsageWindow,
  UsageWindowKind,
} from '../team/events'

export type UsageTone = 'ink' | 'warn' | 'danger'

export interface ProviderThresholds {
  warn: number
  pause: number
}

export interface PaceInfo {
  remainingTimePercent: number
  expectedUsedPercent: number
  remainingQuotaPercent: number
  status: PaceStatus
}

export const WARN_THRESHOLD = 75
export const PAUSE_THRESHOLD = 90
export const MAX_PERCENT = 100
export const OUTER_RING_RADIUS = 20.5
export const MS_PER_SECOND = 1000
export const MS_PER_MINUTE = 60000
export const MINUTES_PER_HOUR = 60
export const HOURS_PER_DAY = 24
export const FIVE_HOUR_MINS = 300
export const WEEKLY_MINS = 10080

export const DEFAULT_THRESHOLDS: ProviderThresholds = {
  warn: WARN_THRESHOLD,
  pause: PAUSE_THRESHOLD,
}

const KNOWN_PROVIDER_META: Record<string, { short: string; name: string }> = {
  claude: { short: 'Claude', name: 'Claude Code' },
  codex: { short: 'Codex', name: 'Codex' },
  opencode: { short: 'OpenCode', name: 'OpenCode' },
  gemini: { short: 'Gemini', name: 'Gemini' },
  antigravity: { short: 'Gemini', name: 'Antigravity' },
}

export const TONE_TEXT: Record<UsageTone | 'muted' | 'accent', string> = {
  ink: 'text-ink',
  warn: 'text-warn-text',
  danger: 'text-danger',
  accent: 'text-accent',
  muted: 'text-muted',
}

export const TONE_BG: Record<UsageTone, string> = {
  ink: 'bg-ink',
  warn: 'bg-warn',
  danger: 'bg-danger',
}

export const TONE_STROKE: Record<UsageTone, string> = {
  ink: 'stroke-ink',
  warn: 'stroke-warn',
  danger: 'stroke-danger',
}

export function usageBgClass(
  percent: number,
  thresholds: ProviderThresholds = DEFAULT_THRESHOLDS,
  usingCredits = false,
): string {
  return usingCredits ? 'bg-accent' : TONE_BG[toneOf(percent, thresholds)]
}

export function usageStrokeClass(
  percent: number,
  thresholds: ProviderThresholds = DEFAULT_THRESHOLDS,
  usingCredits = false,
): string {
  return usingCredits ? 'stroke-accent' : TONE_STROKE[toneOf(percent, thresholds)]
}

export function providerDisplayName(
  usage: Pick<ProviderPlanUsage, 'providerId' | 'label'>,
): string {
  const explicit = usage.label?.trim()
  if (explicit) return explicit
  const known = KNOWN_PROVIDER_META[usage.providerId]
  if (known) return known.name
  return usage.providerId.charAt(0).toUpperCase() + usage.providerId.slice(1)
}

export function providerShortName(
  usage: Pick<ProviderPlanUsage, 'providerId' | 'label'>,
): string {
  const known = KNOWN_PROVIDER_META[usage.providerId]
  if (known) return known.short
  const display = providerDisplayName(usage)
  return display.split(/\s+/)[0] || usage.providerId
}

export function windowKindLabel(kind: UsageWindowKind, customLabel?: string): string {
  const trimmed = customLabel?.trim()
  if (trimmed) return trimmed
  if (kind === 'five-hour') return '5 h'
  if (kind === 'daily') return 'diária'
  if (kind === 'weekly') return 'semanal'
  if (kind === 'monthly') return 'mensal'
  const mins = kind.custom
  if (mins >= MINUTES_PER_HOUR && mins % MINUTES_PER_HOUR === 0) {
    return `${mins / MINUTES_PER_HOUR} h`
  }
  return `${mins} min`
}

export function isWindowExpired(window: ProviderUsageWindow, nowMs: number): boolean {
  return window.resetsAt > 0 && nowMs >= window.resetsAt * MS_PER_SECOND
}

export function allWindowsOf(usage: ProviderPlanUsage): ProviderUsageWindow[] {
  const windows: ProviderUsageWindow[] = []
  if (usage.primary) windows.push(usage.primary)
  if (usage.secondary) windows.push(usage.secondary)
  if (usage.tertiary) windows.push(usage.tertiary)
  if (usage.groups) {
    for (const group of usage.groups) {
      windows.push(group.window)
    }
  }
  return windows
}

export function primaryWindowsOf(usage: ProviderPlanUsage): ProviderUsageWindow[] {
  const windows: ProviderUsageWindow[] = []
  if (usage.primary) windows.push(usage.primary)
  if (usage.secondary) windows.push(usage.secondary)
  if (usage.tertiary) windows.push(usage.tertiary)
  return windows
}

export function isUsingCredits(usage: ProviderPlanUsage): boolean {
  if (usage.creditsAvailable !== true) return false
  return allWindowsOf(usage).some((window) => window.percent >= MAX_PERCENT || Boolean(window.rejected))
}

export function mostRestrictiveWindow(
  usage: ProviderPlanUsage,
  nowMs: number,
): ProviderUsageWindow | null {
  const all = allWindowsOf(usage)
  if (all.length === 0) return null
  const active = all.filter((window) => !isWindowExpired(window, nowMs))
  const pool = active.length > 0 ? active : all
  return pool.reduce((worst, current) => (current.percent > worst.percent ? current : worst))
}

export function paceInfo(window: ProviderUsageWindow, nowMs: number): PaceInfo | null {
  if (!window.durationMins || !window.resetsAt) return null
  const resetsAtMs = window.resetsAt * MS_PER_SECOND
  const durationMs = window.durationMins * MS_PER_MINUTE
  if (durationMs <= 0) return null
  const remainingMs = Math.max(0, resetsAtMs - nowMs)
  const remainingTimePercent = Math.min(MAX_PERCENT, Math.max(0, (remainingMs / durationMs) * MAX_PERCENT))
  const expectedUsedPercent = Math.min(MAX_PERCENT, Math.max(0, MAX_PERCENT - remainingTimePercent))
  const remainingQuotaPercent = Math.max(0, MAX_PERCENT - clampPercent(window.percent))
  const status: PaceStatus = remainingQuotaPercent < remainingTimePercent ? 'above-pace' : 'sustainable'
  return {
    remainingTimePercent,
    expectedUsedPercent,
    remainingQuotaPercent,
    status,
  }
}

export function toneOf(
  percent: number,
  thresholds: ProviderThresholds = DEFAULT_THRESHOLDS,
): UsageTone {
  if (percent >= thresholds.pause) return 'danger'
  if (percent >= thresholds.warn) return 'warn'
  return 'ink'
}

export function stateOf(
  percent: number,
  thresholds: ProviderThresholds = DEFAULT_THRESHOLDS,
  usingCredits = false,
): { label: 'usando créditos' | 'pausado' | 'perto do limite' | 'ok'; tone: 'accent' | 'danger' | 'warn' | 'muted' } {
  if (usingCredits) return { label: 'usando créditos', tone: 'accent' }
  if (percent >= thresholds.pause) return { label: 'pausado', tone: 'danger' }
  if (percent >= thresholds.warn) return { label: 'perto do limite', tone: 'warn' }
  return { label: 'ok', tone: 'muted' }
}

export function clampPercent(percent: number): number {
  return Math.max(0, Math.min(percent, MAX_PERCENT))
}

export function ringDash(percent: number, radius: number): string {
  const circumference = 2 * Math.PI * radius
  return `${((circumference * clampPercent(percent)) / MAX_PERCENT).toFixed(1)} ${circumference.toFixed(1)}`
}

export function renewsLabel(resetsAt: number, nowMs: number): string {
  if (resetsAt <= 0) return 'renovação indefinida'
  const minutes = Math.max(0, Math.ceil((resetsAt * MS_PER_SECOND - nowMs) / MS_PER_MINUTE))
  if (minutes < MINUTES_PER_HOUR) return `renova em ${minutes} min`
  const totalHours = Math.floor(minutes / MINUTES_PER_HOUR)
  if (totalHours >= HOURS_PER_DAY) {
    const days = Math.floor(totalHours / HOURS_PER_DAY)
    const remHours = totalHours % HOURS_PER_DAY
    return remHours > 0 ? `renova em ${days} d ${remHours} h` : `renova em ${days} d`
  }
  const rest = String(minutes % MINUTES_PER_HOUR).padStart(2, '0')
  return `renova em ${totalHours} h ${rest}`
}

export function resetCountdownLabel(resetsAt: number, nowMs: number): string {
  const totalMinutes = Math.max(0, Math.ceil((resetsAt * MS_PER_SECOND - nowMs) / MS_PER_MINUTE))
  const hours = Math.floor(totalMinutes / MINUTES_PER_HOUR)
  const minutes = totalMinutes % MINUTES_PER_HOUR
  if (hours > 0 && minutes > 0) {
    return `em ${hours} ${hours === 1 ? 'hora' : 'horas'} e ${minutes} ${minutes === 1 ? 'minuto' : 'minutos'}`
  }
  if (hours > 0) {
    return `em ${hours} ${hours === 1 ? 'hora' : 'horas'}`
  }
  return `em ${minutes} ${minutes === 1 ? 'minuto' : 'minutos'}`
}

export function updatedLabel(updatedAtMs: number, nowMs: number): string {
  const minutes = Math.max(0, Math.floor((nowMs - updatedAtMs) / MS_PER_MINUTE))
  return minutes < 1 ? 'atualizado agora' : `atualizado há ${minutes} min`
}
