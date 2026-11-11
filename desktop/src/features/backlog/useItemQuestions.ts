import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { useDecisions, type AnswerLogEntry } from '../decisions/useDecisions'
import { DECISION_QUESTION_PREFIX } from './backlogTypes'
import { useBacklog } from './useBacklog'

interface QuestionLink {
  decisionId: number
  itemId: string
}

const questionText = (title: string, detail: string): string => (detail ? `${title} — ${detail}` : title)
const answeredMessage = (title: string): string => `Respondido no item: ${title}`

export const useItemQuestions = defineStore('itemQuestions', () => {
  const backlog = useBacklog()
  const decisions = useDecisions()
  const links = ref<Record<string, QuestionLink>>({})

  function selectedOf(link: QuestionLink, questionId: string): string | undefined {
    return backlog.items.find((item) => item.id === link.itemId)?.questions?.find((question) => question.id === questionId)?.selected
  }

  function isLinked(decisionId: number): boolean {
    return Object.values(links.value).some((link) => link.decisionId === decisionId)
  }

  function syncPending(): void {
    for (const decision of decisions.pending) {
      if (decision.itemId === null || isLinked(decision.id)) continue
      if (backlog.loadedProjectId !== decision.projectId) continue
      if (!backlog.items.some((item) => item.id === decision.itemId)) continue
      const question = {
        id: DECISION_QUESTION_PREFIX + crypto.randomUUID(),
        text: questionText(decision.title, decision.detail),
        options: [...decision.options],
      }
      links.value[question.id] = { decisionId: decision.id, itemId: decision.itemId }
      backlog.addQuestion(decision.itemId, question)
    }
  }

  function reconcile(): void {
    for (const item of backlog.items) {
      const stale = (item.questions ?? [])
        .filter((question) => question.id.startsWith(DECISION_QUESTION_PREFIX) && question.selected === undefined && links.value[question.id] === undefined)
        .map((question) => question.id)
      backlog.removeQuestions(item.id, stale)
    }
  }

  function answer(itemId: string, questionId: string, text: string): void {
    const link = links.value[questionId]
    if (link === undefined) {
      backlog.answer(itemId, questionId, text)
      return
    }
    if (!decisions.pending.some((decision) => decision.id === link.decisionId)) return
    backlog.answer(itemId, questionId, text)
    const title = backlog.items.find((item) => item.id === itemId)?.questions?.find((question) => question.id === questionId)?.text ?? ''
    decisions.answer(link.decisionId, text, answeredMessage(title))
  }

  function applyLogEntry(entry: AnswerLogEntry): void {
    const found = Object.entries(links.value).find(([, link]) => link.decisionId === entry.decisionId)
    if (found === undefined) return
    const [questionId, link] = found
    if (entry.kind === 'undone') {
      backlog.clearAnswer(link.itemId, questionId)
    } else if (selectedOf(link, questionId) !== entry.text) {
      backlog.answer(link.itemId, questionId, entry.text)
    }
  }

  watch(() => decisions.pending, syncPending, { deep: true })
  watch(
    () => decisions.answerLogTotal,
    (total, previous) => decisions.answerLog.slice(-(total - previous)).forEach(applyLogEntry),
  )
  watch(
    () => backlog.loadedProjectId,
    (projectId) => {
      if (projectId === null) return
      reconcile()
      syncPending()
    },
  )
  syncPending()

  return { answer }
})
