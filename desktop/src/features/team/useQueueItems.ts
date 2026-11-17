import { computed } from 'vue'
import { useFeed } from './useFeed'
import { useRequests } from './useRequests'

// Fila da conversa aberta: a do agente na tela dele, ou a do time nas demais telas.
export function useQueueItems() {
  const feed = useFeed()
  const requests = useRequests()
  return computed(() => (feed.screen.kind === 'agent' ? requests.queuedFor(feed.screen.agent) : requests.queue))
}
