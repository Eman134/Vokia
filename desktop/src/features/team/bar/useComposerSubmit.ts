import { ref } from 'vue'
import { DEFAULT_RECIPIENT } from '../agents'
import { mentionSpans } from '../composerMentions'
import { expandCommand, parseLocalCommand } from '../slashCommands'
import type { Agent } from '../events'
import { useConnection } from '../useConnection'
import { useRequests } from '../useRequests'
import { useRequestNotices } from '../../balloon/useRequestNotices'
import { useNavigation } from '../../../shell/useNavigation'
import { screenDestination } from '../screenRecipient'
import { useFeed } from '../useFeed'
import { useRoster } from '../useRoster'
import { useChannels } from '../../channels/useChannels'
import { useComposerAttachments } from '../../attachments/useComposerAttachments'
import { useProviders } from '../../providers/useProviders'

// Rascunho e envio vivem no pai: o campo é desmontado offline, e a restauração após falha precisa sobreviver a isso.
export function useComposerSubmit() {
  const connection = useConnection()
  const requests = useRequests()
  const requestNotices = useRequestNotices()
  const feed = useFeed()
  const navigation = useNavigation()
  const channels = useChannels()
  const roster = useRoster()
  const providers = useProviders()
  const draft = ref('')
  const confirmed = ref(new Set<Agent>())
  const composerAttachments = useComposerAttachments()

  async function submit(): Promise<void> {
    const text = draft.value.trim()
    if ((text === '' && composerAttachments.attachments.value.length === 0) || connection.checking || composerAttachments.saving.value) {
      return
    }
    const localCommand = parseLocalCommand(text)
    if (localCommand?.action === 'setup') {
      draft.value = ''
      confirmed.value = new Set()
      providers.openSetup(localCommand.arg)
      return
    }
    const mention = mentionSpans(draft.value, confirmed.value)[0]?.agent ?? null
    const { to, channelId } = screenDestination(
      navigation.page,
      feed.screen,
      channels.channels,
      roster.agents.map((profile) => profile.id),
      mention,
    )
    const expandedCommand = expandCommand(text)
    const pending = draft.value
    // O watch do rascunho esvazia as menções confirmadas; guardá-las antes permite restaurá-las na falha.
    const pendingConfirmed = confirmed.value
    const batch = composerAttachments.take()
    draft.value = ''
    // A rejeição do comando chega depois do envio: o usuário pode já ter digitado de novo, e isso não se sobrescreve.
    const restore = (): void => {
      if (draft.value === '') {
        draft.value = pending
        confirmed.value = pendingConfirmed
      }
      if (composerAttachments.attachments.value.length === 0) {
        composerAttachments.restore(batch)
      }
    }
    if (
      !(await requests.send(expandedCommand?.text ?? text, expandedCommand ? (to ?? DEFAULT_RECIPIENT) : to, {
        channelId: channelId ?? undefined,
        attachments: batch.ids,
        attachmentsProjectId: batch.projectId,
        onEnqueued: requestNotices.announceSent,
        onQueued: requestNotices.markBackendQueued,
        onRejected: restore,
      }))
    ) {
      restore()
    }
  }

  return { draft, confirmed, attachments: composerAttachments, submit }
}
