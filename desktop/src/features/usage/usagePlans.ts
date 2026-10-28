export type PlanId = 'cc'
export type UsageTone = 'ink' | 'warn' | 'danger'

export interface PlanInfo {
  id: PlanId
  short: string
  name: string
}

export const WARN_THRESHOLD = 75
export const PAUSE_THRESHOLD = 90
export const MAX_PERCENT = 100
export const OUTER_RING_RADIUS = 20.5
export const MS_PER_SECOND = 1000
export const MS_PER_MINUTE = 60000
export const MINUTES_PER_HOUR = 60

export const PLANS: readonly PlanInfo[] = [{ id: 'cc', short: 'Claude', name: 'Claude Code' }]

export const TONE_TEXT: Record<UsageTone | 'muted', string> = {
  ink: 'text-ink',
  warn: 'text-warn',
  danger: 'text-danger',
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

export function toneOf(percent: number): UsageTone {
  if (percent >= PAUSE_THRESHOLD) return 'danger'
  if (percent >= WARN_THRESHOLD) return 'warn'
  return 'ink'
}

export function stateOf(percent: number): { label: 'pausado' | 'perto do limite' | 'ok'; tone: 'danger' | 'warn' | 'muted' } {
  if (percent >= PAUSE_THRESHOLD) return { label: 'pausado', tone: 'danger' }
  if (percent >= WARN_THRESHOLD) return { label: 'perto do limite', tone: 'warn' }
  return { label: 'ok', tone: 'muted' }
}

export function clampPercent(percent: number): number {
  return Math.min(percent, MAX_PERCENT)
}

export function ringDash(percent: number, radius: number): string {
  const circumference = 2 * Math.PI * radius
  return `${((circumference * clampPercent(percent)) / MAX_PERCENT).toFixed(1)} ${circumference.toFixed(1)}`
}

export function renewsLabel(resetsAt: number, nowMs: number): string {
  const minutes = Math.max(0, Math.ceil((resetsAt * MS_PER_SECOND - nowMs) / MS_PER_MINUTE))
  if (minutes < MINUTES_PER_HOUR) return `renova em ${minutes} min`
  const hours = Math.floor(minutes / MINUTES_PER_HOUR)
  const rest = String(minutes % MINUTES_PER_HOUR).padStart(2, '0')
  return `renova em ${hours} h ${rest}`
}

export function updatedLabel(updatedAtMs: number, nowMs: number): string {
  const minutes = Math.floor((nowMs - updatedAtMs) / MS_PER_MINUTE)
  return minutes < 1 ? 'atualizado agora' : `atualizado há ${minutes} min`
}
