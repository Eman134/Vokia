import type { VokiaUsageLimit } from '../team/events'
import { MS_PER_SECOND } from './usagePlans'

const LOCALE = 'pt-BR'
const TIME_FORMAT: Intl.DateTimeFormatOptions = { hour: '2-digit', minute: '2-digit' }
const DAY_MONTH_FORMAT: Intl.DateTimeFormatOptions = { day: '2-digit', month: '2-digit' }

export function usageLimitText(limit: Pick<VokiaUsageLimit, 'window' | 'resetsAt'>): string {
  const reset = new Date(limit.resetsAt * MS_PER_SECOND)
  const time = reset.toLocaleTimeString(LOCALE, TIME_FORMAT)
  if (limit.window === 'five-hour') {
    return `Limite de uso atingido: de 5 horas, libera às ${time}`
  }
  const day = reset.toLocaleDateString(LOCALE, DAY_MONTH_FORMAT)
  return `Limite de uso atingido: semanal, libera em ${day} às ${time}`
}
