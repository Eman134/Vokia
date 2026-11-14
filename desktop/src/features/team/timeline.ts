import type { FeedNotice, FeedPermissionResult, VokiaError, VokiaMessage } from './events'

export type FeedEntry = VokiaMessage | VokiaError | FeedNotice | FeedPermissionResult

export type TimelineItem =
  | { type: 'entry'; start: number; entry: FeedEntry }
  | { type: 'group'; start: number; messages: VokiaMessage[] }

export function isBetweenAgents(message: VokiaMessage): boolean {
  return message.from !== 'user' && message.to !== 'user'
}

export function buildTimeline(feed: readonly FeedEntry[]): TimelineItem[] {
  const items: TimelineItem[] = []
  feed.forEach((entry, index) => {
    const last = items.at(-1)
    if (entry.type !== 'message' || !isBetweenAgents(entry)) {
      items.push({ type: 'entry', start: index, entry })
    } else if (last?.type === 'group') {
      last.messages.push(entry)
    } else {
      items.push({ type: 'group', start: index, messages: [entry] })
    }
  })
  return items
}
