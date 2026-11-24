import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { useBacklog } from '../backlog/useBacklog'
import { useDecisions } from '../decisions/useDecisions'
import { useProjects } from '../projects/useProjects'
import { CONVERSATIONS_PAGE } from '../team/screenRecipient'
import { screenEntries, type ConversationScreen } from '../team/timeline'
import { useFeed } from '../team/useFeed'
import { useThreads } from '../team/useThreads'
import { useNavigation } from '../../shell/useNavigation'
import { GENERAL_CHANNEL_NAME } from './channelRouting'
import { latestIncomingTime, pendingDecisionKeys, unreadKeyOf, type UnreadKey, type UnreadState } from './unread'
import { useChannels } from './useChannels'

const STORAGE_KEY_PREFIX = 'vokia.unread.'

interface ReadRecord {
  // Tudo que chegou antes do primeiro registro do projeto conta como lido.
  baseline: number
  lastRead: Record<UnreadKey, number>
}

function storageKey(projectId: number): string {
  return `${STORAGE_KEY_PREFIX}${projectId}`
}

function isReadRecord(value: unknown): value is ReadRecord {
  if (typeof value !== 'object' || value === null) return false
  const record = value as Partial<ReadRecord>
  return typeof record.baseline === 'number' && typeof record.lastRead === 'object' && record.lastRead !== null
}

function readStored(projectId: number): ReadRecord | null {
  try {
    const raw = localStorage.getItem(storageKey(projectId))
    if (raw === null) return null
    const parsed: unknown = JSON.parse(raw)
    return isReadRecord(parsed) ? parsed : null
  } catch {
    // Registro ilegível ou armazenamento indisponível: recomeça com tudo lido.
    return null
  }
}

function writeStored(projectId: number, record: ReadRecord): void {
  try {
    localStorage.setItem(storageKey(projectId), JSON.stringify(record))
  } catch {
    // Sem armazenamento, a leitura vale só nesta sessão.
  }
}

export const useUnread = defineStore('unread', () => {
  const feed = useFeed()
  const channels = useChannels()
  const threads = useThreads()
  const backlog = useBacklog()
  const decisions = useDecisions()
  const projects = useProjects()
  const navigation = useNavigation()
  const records = ref<Record<number, ReadRecord>>({})

  const projectId = computed(() => projects.selected?.id ?? null)

  watch(
    projectId,
    (id) => {
      if (id === null || records.value[id] !== undefined) return
      const record = readStored(id) ?? { baseline: Date.now(), lastRead: {} }
      records.value[id] = record
      writeStored(id, record)
    },
    { immediate: true },
  )

  const record = computed<ReadRecord | undefined>(() =>
    projectId.value === null ? undefined : records.value[projectId.value],
  )

  const screens = computed<ConversationScreen[]>(() => [
    { kind: 'channel', channel: GENERAL_CHANNEL_NAME },
    ...channels.channels.map((channel): ConversationScreen => ({ kind: 'channel', channel: channel.id })),
    { kind: 'triage' },
    ...threads.threads.map((thread): ConversationScreen => ({ kind: 'thread', itemId: thread.itemId })),
  ])

  const latestByKey = computed<Map<UnreadKey, number>>(() => {
    const latest = new Map<UnreadKey, number>()
    for (const screen of screens.value) {
      const key = unreadKeyOf(screen)
      const time = latestIncomingTime(screenEntries(feed.feed, screen, channels.channels, undefined, backlog.triagingIds))
      if (key !== null && time !== null) latest.set(key, time)
    }
    return latest
  })

  const decisionKeys = computed(() =>
    pendingDecisionKeys(decisions.pending, feed.permissions, {
      threadIds: new Set(threads.threads.map((thread) => thread.itemId)),
      triagingIds: backlog.triagingIds,
      agentItems: feed.stateItem,
    }),
  )

  function stateOf(screen: ConversationScreen): UnreadState {
    const key = unreadKeyOf(screen)
    if (key === null) return 'read'
    if (decisionKeys.value.has(key)) return 'decision'
    const latest = latestByKey.value.get(key)
    const current = record.value
    if (latest === undefined || current === undefined) return 'read'
    return latest > (current.lastRead[key] ?? current.baseline) ? 'unread' : 'read'
  }

  function markRead(key: UnreadKey): void {
    const latest = latestByKey.value.get(key)
    const current = record.value
    if (latest === undefined || current === undefined || projectId.value === null) return
    if ((current.lastRead[key] ?? current.baseline) >= latest) return
    current.lastRead[key] = latest
    writeStored(projectId.value, current)
  }

  // A tela aberta em Conversas nunca fica não lida: marca ao abrir e a cada novidade enquanto ela está à vista.
  const openKey = computed(() => (navigation.page === CONVERSATIONS_PAGE ? unreadKeyOf(feed.screen) : null))
  watch(
    [openKey, () => (openKey.value === null ? undefined : latestByKey.value.get(openKey.value))],
    ([key]) => {
      if (key !== null) markRead(key)
    },
    { immediate: true },
  )

  return { stateOf, markRead }
})
