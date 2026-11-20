import type { VokiaUsageLimit } from '../team/events'
import { resetCountdownLabel } from './usagePlans'

export function usageLimitText(
  limit: Pick<VokiaUsageLimit, 'window' | 'resetsAt'>,
  nowMs = Date.now(),
): string {
  if (limit.resetsAt > 0) {
    return `Limite esgotado · volta ${resetCountdownLabel(limit.resetsAt, nowMs)}`
  }
  return 'Limite esgotado'
}
