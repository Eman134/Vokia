import { ref } from 'vue'
import { DEFAULT_RECIPIENT } from '../agents'
import { mentionSpans } from '../composerMentions'
import { expandCommand, parseLocalCommand } from '../slashCommands'
import { useBtw } from './useBtw'
import { agentName } from '../agents'
import { useProjects } from '../../projects/useProjects'
import { useTriage } from '../../backlog/useTriage'
import { useRunItem } from '../../backlog/useRunItem'
import { isUnassigned } from '../../backlog/backlogTypes'
import { useAgentControl } from '../../panel/useAgentControl'
import { saveProjectMemory } from '../../memory/learningApi'
import type { Agent } from '../events'
import { useConnection } from '../useConnection'
import { useRequests } from '../useRequests'
import { useRequestNotices } from '../../balloon/useRequestNotices'
import { useNavigation } from '../../../shell/useNavigation'
import { useBacklog } from '../../backlog/useBacklog'
import { useThreads } from '../useThreads'
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
  const deliveryMode = ref<'auto' | 'now' | 'queue'>('auto')
  const composerAttachments = useComposerAttachments()
  const projects = useProjects()

  async function submit(): Promise<void> {
    const text = draft.value.trim()
    if ((text === '' && composerAttachments.attachments.value.length === 0) || connection.checking || composerAttachments.saving.value) {
      return
    }
    const localCommand = parseLocalCommand(text)
    if (localCommand?.action === 'btw') {
      if (!localCommand.arg) {
        feed.notify('Escreva sua pergunta após /btw.')
        return
      }
      const mention = mentionSpans(draft.value, confirmed.value)[0]?.agent ?? null
      const threads = useThreads()
      const threadAssignee = feed.screen.kind === 'thread' ? (threads.find(feed.screen.itemId)?.assignee ?? null) : null
      const dest = screenDestination(
        navigation.page,
        feed.screen,
        channels.channels,
        roster.agents.map((profile) => profile.id),
        mention,
        threadAssignee,
      )
      draft.value = ''
      confirmed.value = new Set()
      void useBtw().ask({
        projectId: projects.selected?.id ?? null,
        agent: dest.to,
        question: localCommand.arg,
      })
      return
    }
    if (localCommand?.action === 'setup') {
      draft.value = ''
      confirmed.value = new Set()
      providers.openSetup(localCommand.arg)
      return
    }
    if (localCommand?.action === 'triar' || localCommand?.action === 'iniciar') {
      const projectId = projects.selected?.id
      if (projectId === undefined) {
        feed.notify('Selecione um projeto antes de executar este comando.')
        return
      }
      const backlog = useBacklog()
      if (backlog.loadedProjectId !== projectId) {
        await backlog.load(projectId)
      }
      const targetColumn = localCommand.action === 'triar' ? 'Backlog' : 'Ready'
      const normalizedArg = localCommand.arg.replace(/^#/, '').trim().toLowerCase()
      const candidates = backlog.items.filter((i) => i.column === targetColumn)
      const item = normalizedArg
        ? candidates.find(
            (i) => i.id.toLowerCase() === normalizedArg || i.title.toLowerCase().includes(normalizedArg),
          )
        : candidates[0]
      draft.value = ''
      confirmed.value = new Set()
      if (!item) {
        feed.notify(`Nenhum item em ${targetColumn} encontrado${localCommand.arg ? ` para "${localCommand.arg}"` : ''}.`)
        return
      }
      if (localCommand.action === 'triar') {
        if (!isUnassigned(item)) {
          backlog.open(item.id)
          feed.notify(`${item.id} já possui responsável; ficha aberta.`)
          return
        }
        await useTriage().requestTriage(item)
        feed.notify(`Triagem iniciada para ${item.id} (${item.title}).`)
      } else {
        const runItem = useRunItem()
        if (!runItem.canRun(item.id)) {
          backlog.open(item.id)
          feed.notify(`${item.id} não pode ser iniciado agora; ficha aberta.`)
          return
        }
        await runItem.run(item.id)
        feed.notify(`Execução iniciada para ${item.id} (${item.title}).`)
      }
      return
    }
    if (localCommand?.action === 'pausar') {
      const projectId = projects.selected?.id
      if (projectId === undefined) {
        feed.notify('Selecione um projeto para pausar ou retomar agentes.')
        return
      }
      const control = useAgentControl()
      const mention = mentionSpans(draft.value, confirmed.value)[0]?.agent ?? null
      const arg = localCommand.arg.replace(/^@/, '').trim().toLowerCase()
      draft.value = ''
      confirmed.value = new Set()
      if (mention || arg) {
        const target =
          mention ??
          roster.agents.find((a) => a.id.toLowerCase() === arg || a.name.toLowerCase() === arg)?.id
        if (!target) {
          feed.notify(`Agente "${localCommand.arg}" não encontrado.`)
          return
        }
        const pausing = !control.isPaused(target, projectId)
        await (pausing ? control.pause(target) : control.resume(target))
        feed.notify(`${agentName(target)} ${pausing ? 'pausado' : 'retomado'}.`)
      } else {
        const anyActive = roster.agents.some((a) => !control.isPaused(a.id, projectId))
        for (const a of roster.agents) {
          if (anyActive && !control.isPaused(a.id, projectId)) {
            await control.pause(a.id)
          } else if (!anyActive && control.isPaused(a.id, projectId)) {
            await control.resume(a.id)
          }
        }
        feed.notify(anyActive ? 'Time pausado neste projeto.' : 'Time retomado neste projeto.')
      }
      return
    }
    if (localCommand?.action === 'lembrar') {
      const projectId = projects.selected?.id
      if (projectId === undefined) {
        feed.notify('Selecione um projeto para salvar uma memória.')
        return
      }
      if (!localCommand.arg) {
        feed.notify('Escreva o fato a guardar após /lembrar.')
        return
      }
      draft.value = ''
      confirmed.value = new Set()
      try {
        const saved = await saveProjectMemory(projectId, localCommand.arg)
        feed.notify(`Memória salva: "${saved.fact}"`)
      } catch (error) {
        feed.feed.push({ type: 'error', message: String(error) })
      }
      return
    }
    const mention = mentionSpans(draft.value, confirmed.value)[0]?.agent ?? null
    const threads = useThreads()
    const backlog = useBacklog()
    const triage = useTriage()
    const threadAssignee = feed.screen.kind === 'thread' ? (threads.find(feed.screen.itemId)?.assignee ?? null) : null
    const triageItem =
      feed.screen.kind === 'triage'
        ? (backlog.items.find((item) => item.id === triage.selectedTriageId && backlog.triagingIds.has(item.id)) ??
          backlog.items.find((item) => backlog.triagingIds.has(item.id)) ??
          null)
        : null
    const triageTarget = triageItem
      ? {
          itemId: triageItem.id,
          agent: isUnassigned(triageItem) ? null : triageItem.who,
          label: triageItem.publicId ?? triageItem.id,
        }
      : null
    const { to, channelId, itemId } = screenDestination(
      navigation.page,
      feed.screen,
      channels.channels,
      roster.agents.map((profile) => profile.id),
      mention,
      threadAssignee,
      triageTarget,
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
    const forceMode = deliveryMode.value === 'auto' ? undefined : deliveryMode.value
    deliveryMode.value = 'auto'
    if (
      !(await requests.send(expandedCommand?.text ?? text, expandedCommand ? (to ?? DEFAULT_RECIPIENT) : to, {
        channelId: channelId ?? undefined,
        itemId: itemId ?? undefined,
        attachments: batch.ids,
        attachmentsProjectId: batch.projectId,
        force: forceMode,
        onEnqueued: requestNotices.announceSent,
        onQueued: requestNotices.markBackendQueued,
        onRejected: restore,
      }))
    ) {
      restore()
    }
  }

  return { draft, confirmed, deliveryMode, attachments: composerAttachments, submit }
}
