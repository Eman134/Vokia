import type { BalloonItem } from '../balloon/balloonTypes'
import type { MascotLook } from '../brand/mascotArt'
import { PAUSE_THRESHOLD, WARN_THRESHOLD, renewsLabel } from './usagePlans'

export type LimitLevel = 'none' | 'warn75' | 'pause90'
export type LimitNoticeKind = Exclude<LimitLevel, 'none'> | 'renewed'

export const LIMIT_LEVELS: readonly LimitLevel[] = ['none', 'warn75', 'pause90']

export const LIMIT_MASCOT: Record<LimitNoticeKind, MascotLook> = {
  warn75: 'precisa-de-voce',
  pause90: 'pausado',
  renewed: 'atualizacao-pronta',
}

export interface LimitContext {
  fiveHourPercent: number
  sevenDayPercent: number
  resetsAt: number
  nowMs: number
}

export function levelOf(percent: number | null): LimitLevel {
  if (percent === null) return 'none'
  if (percent >= PAUSE_THRESHOLD) return 'pause90'
  if (percent >= WARN_THRESHOLD) return 'warn75'
  return 'none'
}

export function buildLimitBalloon(kind: LimitNoticeKind, context: LimitContext): BalloonItem {
  const { fiveHourPercent, sevenDayPercent, resetsAt, nowMs } = context
  const id = `limit:${kind}:${resetsAt}`
  const mascot = LIMIT_MASCOT[kind]
  const source = 'uso do plano'

  if (kind === 'warn75') {
    return {
      id,
      kind: 'limit',
      source,
      title: '75%',
      text: `Claude em ${fiveHourPercent}% da janela de 5 h. ${renewsLabel(resetsAt, nowMs)}. Passar a liderança para Sonnet até renovar?`,
      mascot,
      actions: [
        { id: 'economy', label: 'Modo econômico', disabled: true },
        { id: 'later', label: 'Agora não' },
      ],
    }
  }

  if (kind === 'pause90') {
    return {
      id,
      kind: 'limit',
      source,
      title: 'Time pausado pelo limite',
      text: `O Claude chegou a ${fiveHourPercent}% da janela de 5 h. O time volta sozinho quando a janela renovar (${renewsLabel(resetsAt, nowMs)}).`,
      mascot,
      actions: [
        { id: 'codex', label: 'Continuar com o Codex', disabled: true },
        { id: 'wait', label: 'Esperar' },
      ],
    }
  }

  return {
    id,
    kind: 'limit',
    source,
    title: 'Janela renovada, o time voltou',
    text: `Claude em ${fiveHourPercent}% de 5 h e ${sevenDayPercent}% da semana.`,
    mascot,
    actions: [{ id: 'ok', label: 'Ok' }],
  }
}
