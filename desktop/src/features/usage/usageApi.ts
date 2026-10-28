import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { VOKIA_EVENT, type VokiaEvent, type VokiaPlanUsage } from '../team/events'

export type { UnlistenFn }

export function listenPlanUsage(onUsage: (usage: VokiaPlanUsage) => void): Promise<UnlistenFn> {
  return listen<VokiaEvent>(VOKIA_EVENT, (event) => {
    const payload = event.payload
    if (payload.type === 'plan-usage') {
      onUsage(payload)
    }
  })
}
