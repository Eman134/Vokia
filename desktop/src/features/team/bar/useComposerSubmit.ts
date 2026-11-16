import { ref } from 'vue'
import { DEFAULT_RECIPIENT } from '../agents'
import { mentionSpans } from '../composerMentions'
import { expandCommand } from '../slashCommands'
import type { Agent } from '../events'
import { useConnection } from '../useConnection'
import { useRequests } from '../useRequests'
import { useRequestNotices } from '../../balloon/useRequestNotices'
import { useNavigation } from '../../../shell/useNavigation'
import { screenAgent } from '../screenRecipient'
import { useFeed } from '../useFeed'

// Rascunho e envio vivem no pai: o campo é desmontado offline, e a restauração após falha precisa sobreviver a isso.
export function useComposerSubmit() {
  const connection = useConnection()
  const requests = useRequests()
  const requestNotices = useRequestNotices()
  const feed = useFeed()
  const navigation = useNavigation()
  const draft = ref('')
  const confirmed = ref(new Set<Agent>())

  async function submit(): Promise<void> {
    const text = draft.value.trim()
    if (text === '' || connection.checking) {
      return
    }
    const to = mentionSpans(draft.value, confirmed.value)[0]?.agent ?? screenAgent(navigation.page, feed.screen)
    const expandedCommand = expandCommand(text)
    const pending = draft.value
    // O watch do rascunho esvazia as menções confirmadas; guardá-las antes permite restaurá-las na falha.
    const pendingConfirmed = confirmed.value
    draft.value = ''
    if (
      !(await requests.send(expandedCommand?.text ?? text, expandedCommand ? (to ?? DEFAULT_RECIPIENT) : to, {
        onEnqueued: requestNotices.announceSent,
        onQueued: requestNotices.markBackendQueued,
      }))
    ) {
      draft.value = pending
      confirmed.value = pendingConfirmed
    }
  }

  return { draft, confirmed, submit }
}
