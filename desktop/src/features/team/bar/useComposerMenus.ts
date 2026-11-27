import { computed, ref, useId, watch, type Ref } from 'vue'
import { normalizeName, useRoster } from '../useRoster'
import type { AgentProfile } from '../teamApi'
import type { Agent } from '../events'
import { useBacklog } from '../../backlog/useBacklog'
import type { BacklogItem } from '../../backlog/backlogTypes'
import {
  findTaskTrigger,
  findTrigger,
  insertMention,
  insertTaskMention,
  MENTION_TRIGGER,
  replaceTrigger,
  type TriggerMatch,
} from '../composerMentions'
import { COMMAND_TRIGGER, findCommandTrigger, SLASH_COMMANDS, type SlashCommand } from '../slashCommands'

const MAX_TASK_MENTION_RESULTS = 8

interface ComposerMenusInput {
  draft: Ref<string>
  caret: Ref<number>
  confirmed: Ref<Set<Agent>>
  locked: Readonly<Ref<boolean>>
  placeCaret: (position: number) => Promise<void>
}

export function useComposerMenus({ draft, caret, confirmed, locked, placeCaret }: ComposerMenusInput) {
  const roster = useRoster()
  const backlog = useBacklog()
  const listboxId = useId()
  const menuDismissed = ref(false)
  const highlightedIndex = ref(0)

  const mentionMatch = computed<TriggerMatch | null>(() => findTrigger(draft.value, caret.value, MENTION_TRIGGER))
  const taskMatch = computed<TriggerMatch | null>(() => findTaskTrigger(draft.value, caret.value))
  const commandMatch = computed<TriggerMatch | null>(() => findCommandTrigger(draft.value, caret.value))
  const menuKind = computed<'command' | 'mention' | 'task' | null>(() =>
    commandMatch.value ? 'command' : mentionMatch.value ? 'mention' : taskMatch.value ? 'task' : null,
  )
  const activeMatch = computed(() => commandMatch.value ?? mentionMatch.value ?? taskMatch.value)
  const mentionQuery = computed<string | null>(() => mentionMatch.value?.query ?? null)
  const mentionMatches = computed<AgentProfile[]>(() => {
    const query = mentionQuery.value
    if (query === null) {
      return []
    }
    const prefix = normalizeName(query)
    return roster.agents.filter((agent) => normalizeName(agent.name).startsWith(prefix))
  })
  const taskMatches = computed<BacklogItem[]>(() => {
    const match = taskMatch.value
    if (match === null) {
      return []
    }
    const query = match.query.trim().toLowerCase()
    const items = backlog.items.filter((item) => !item.error)
    if (query === '') {
      return items.slice(0, MAX_TASK_MENTION_RESULTS)
    }
    const scored = items
      .map((item) => {
        const pub = item.publicId?.toLowerCase() ?? ''
        const id = item.id.toLowerCase()
        const title = item.title.toLowerCase()
        if (pub === query || id === query) return { item, rank: 0 }
        if (pub.startsWith(query) || id.startsWith(query)) return { item, rank: 1 }
        if (pub.includes(query) || id.includes(query)) return { item, rank: 2 }
        if (title.includes(query)) return { item, rank: 3 }
        return null
      })
      .filter((entry): entry is { item: BacklogItem; rank: number } => entry !== null)
    scored.sort((a, b) => a.rank - b.rank)
    return scored.slice(0, MAX_TASK_MENTION_RESULTS).map((entry) => entry.item)
  })
  const commandMatches = computed<SlashCommand[]>(() => {
    const match = commandMatch.value
    if (match === null) {
      return []
    }
    const prefix = match.query.toLowerCase()
    return SLASH_COMMANDS.filter((command) => command.id.startsWith(prefix))
  })
  const menuCount = computed(() => {
    if (menuKind.value === 'command') return commandMatches.value.length
    if (menuKind.value === 'task') return taskMatches.value.length
    return mentionMatches.value.length
  })
  const highlighted = computed<AgentProfile | SlashCommand | BacklogItem | undefined>(() => {
    if (menuKind.value === 'command') return commandMatches.value.at(highlightedIndex.value)
    if (menuKind.value === 'task') return taskMatches.value.at(highlightedIndex.value)
    return mentionMatches.value.at(highlightedIndex.value)
  })
  const menuOpen = computed(() => activeMatch.value !== null && !menuDismissed.value && !locked.value)
  const activeOptionId = computed(() =>
    menuOpen.value && highlighted.value ? `${listboxId}-${highlighted.value.id}` : undefined,
  )

  watch([menuKind, () => activeMatch.value?.query], () => {
    menuDismissed.value = false
    highlightedIndex.value = 0
  })

  function applyMention(next: { text: string; caret: number }, agent: AgentProfile): void {
    draft.value = next.text
    caret.value = next.caret
    confirmed.value = new Set(confirmed.value).add(agent.id)
    void placeCaret(next.caret)
  }

  function chooseMentionAgent(agent: AgentProfile): void {
    const trigger = mentionMatch.value
    if (trigger === null) {
      return
    }
    applyMention(insertMention(draft.value, trigger, caret.value, agent.name), agent)
  }

  function chooseMentionTask(task: BacklogItem): void {
    const trigger = taskMatch.value
    if (trigger === null) {
      return
    }
    const next = insertTaskMention(draft.value, trigger, caret.value, task.publicId ?? task.id)
    draft.value = next.text
    caret.value = next.caret
    void placeCaret(next.caret)
  }

  function chooseCommand(command: SlashCommand): void {
    const trigger = commandMatch.value
    if (trigger === null || !command.enabled) {
      return
    }
    const next = replaceTrigger(draft.value, trigger, caret.value, `${COMMAND_TRIGGER}${command.id} `)
    draft.value = next.text
    caret.value = next.caret
    void placeCaret(next.caret)
  }

  function chooseHighlighted(item: AgentProfile | SlashCommand | BacklogItem): void {
    if ('role' in item) {
      chooseMentionAgent(item)
    } else if ('title' in item) {
      chooseMentionTask(item)
    } else {
      chooseCommand(item)
    }
  }

  function insertAtCaret(agent: AgentProfile): void {
    const before = draft.value.slice(0, caret.value)
    const separator = before === '' || /\s$/.test(before) ? '' : ' '
    const start = before.length + separator.length
    const text = before + separator + draft.value.slice(caret.value)
    applyMention(insertMention(text, { start, query: '' }, start, agent.name), agent)
  }

  function dismissMenu(): void {
    menuDismissed.value = true
  }

  function moveHighlight(step: number): void {
    const count = menuCount.value
    if (count === 0) {
      return
    }
    highlightedIndex.value = (highlightedIndex.value + step + count) % count
  }

  function handleMenuKey(event: KeyboardEvent): void {
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault()
        moveHighlight(1)
        break
      case 'ArrowUp':
        event.preventDefault()
        moveHighlight(-1)
        break
      case 'Enter':
      case 'Tab': {
        const item = highlighted.value
        if (item === undefined && event.key === 'Tab') {
          break
        }
        // Enter com a lista aberta nunca envia, mesmo sem resultado.
        event.preventDefault()
        if (item !== undefined) {
          chooseHighlighted(item)
        }
        break
      }
      case 'Escape':
        event.preventDefault()
        dismissMenu()
        break
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    if (menuOpen.value) {
      handleMenuKey(event)
    }
  }

  return {
    menuKind,
    activeMatch,
    mentionMatches,
    taskMatches,
    commandMatches,
    menuCount,
    highlightedIndex,
    menuOpen,
    activeOptionId,
    listboxId,
    chooseMentionAgent,
    chooseMentionTask,
    chooseCommand,
    insertAtCaret,
    dismissMenu,
    onKeydown,
  }
}
