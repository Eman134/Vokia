import { computed, type ComputedRef } from 'vue'
import { EMPTY_ITEM_USAGE, itemUsageIn, type ItemUsage } from './itemUsage'
import { useFeed } from './useFeed'

// Reativo: cada mensagem nova no feed recalcula, então o total acompanha os agentes trabalhando. Reutilizável pelo card do kanban (item-20).
export function useItemUsage(itemId: () => string | undefined): ComputedRef<ItemUsage> {
  const feed = useFeed()
  return computed(() => {
    const id = itemId()
    return id === undefined ? EMPTY_ITEM_USAGE : itemUsageIn(feed.feed, id)
  })
}
