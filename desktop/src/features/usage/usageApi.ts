import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  VOKIA_EVENT,
  type PlanUsageSnapshot,
  type ProviderPlanUsage,
  type VokiaEvent,
  type VokiaPlanUsage,
} from '../team/events'

export type { UnlistenFn }

const GET_PLAN_USAGE_COMMAND = 'get_plan_usage'
const REFRESH_PLAN_USAGE_COMMAND = 'refresh_plan_usage'

export function getPlanUsage(providerId?: string): Promise<PlanUsageSnapshot> {
  return invoke<PlanUsageSnapshot>(GET_PLAN_USAGE_COMMAND, {
    providerId: providerId ?? null,
  })
}

export function refreshPlanUsage(providerId?: string): Promise<PlanUsageSnapshot> {
  return invoke<PlanUsageSnapshot>(REFRESH_PLAN_USAGE_COMMAND, {
    providerId: providerId ?? null,
  })
}

export function listenUsageUpdates(handlers: {
  onProviderUsage: (usage: ProviderPlanUsage) => void
  onLegacyPlanUsage?: (usage: VokiaPlanUsage) => void
}): Promise<UnlistenFn> {
  return listen<VokiaEvent>(VOKIA_EVENT, (event) => {
    const payload = event.payload
    if (payload.type === 'provider-usage-updated') {
      handlers.onProviderUsage(payload.usage)
    } else if (payload.type === 'plan-usage' && handlers.onLegacyPlanUsage) {
      handlers.onLegacyPlanUsage(payload)
    }
  })
}
