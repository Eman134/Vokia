import { computed, ref, useId, watch, type Ref } from 'vue'
import { normalizeName, useRoster } from '../useRoster'
import type { AgentProfile } from '../teamApi'
import type { Agent } from '../events'
import {
  findTrigger,
  insertMention,
  MENTION_TRIGGER,
  replaceTrigger,
  type TriggerMatch,
} from '../composerMentions'
import { COMMAND_TRIGGER, findCommandTrigger, SLASH_COMMANDS, type SlashCommand } from '../slashCommands'

interface ComposerMenusInput {
  draft: Ref<string>
  caret: Ref<number>
  confirmed: Ref<Set<Agent>>
  locked: Readonly<Ref<boolean>>
  placeCaret: (position: number) => Promise<void>
}

export function useComposerMenus({ draft, caret, confirmed, locked, placeCaret }: ComposerMenusInput) {
  const roster = useRoster()
  const listboxId = useId()
  const menuDismissed = ref(false)
  const highlightedIndex = ref(0)

  const mentionMatch = computed<TriggerMatch | null>(() => findTrigger(draft.value, caret.value, MENTION_TRIGGER))
  const commandMatch = computed<TriggerMatch | null>(() => findCommandTrigger(draft.value, caret.value))
  const menuKind = computed<'command' | 'mention' | null>(() =>
    commandMatch.value ? 'command' : mentionMatch.value ? 'mention' : null,
  )
  const activeMatch = computed(() => commandMatch.value ?? mentionMatch.value)
  const mentionQuery = computed<string | null>(() => mentionMatch.value?.query ?? null)
  const mentionMatches = computed<AgentProfile[]>(() => {
    const query = mentionQuery.value
    if (query === null) {
      return []
    }
    const prefix = normalizeName(query)
    return roster.agents.filter((agent) => normalizeName(agent.name).startsWith(prefix))
  })
  const commandMatches = computed<SlashCommand[]>(() => {
    const match = commandMatch.value
    if (match === null) {
      return []
    }
    const prefix = match.query.toLowerCase()
    return SLASH_COMMANDS.filter((command) => command.id.startsWith(prefix))
  })
  const menuCount = computed(() => (menuKind.value === 'command' ? commandMatches.value : mentionMatches.value).length)
  const highlighted = computed<AgentProfile | SlashCommand | undefined>(() =>
    menuKind.value === 'command'
      ? commandMatches.value.at(highlightedIndex.value)
      : mentionMatches.value.at(highlightedIndex.value),
  )
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

  function chooseHighlighted(item: AgentProfile | SlashCommand): void {
    if ('role' in item) {
      chooseMentionAgent(item)
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
    commandMatches,
    menuCount,
    highlightedIndex,
    menuOpen,
    activeOptionId,
    listboxId,
    chooseMentionAgent,
    chooseCommand,
    insertAtCaret,
    dismissMenu,
    onKeydown,
  }
}
