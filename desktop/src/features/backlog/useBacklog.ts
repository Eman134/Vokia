import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { DEFAULT_COLUMN, MOVABLE_COLUMNS, type BacklogColumn, type BacklogItem, type BacklogTag } from './backlogTypes'

export const useBacklog = defineStore('backlog', () => {
  const items = ref<BacklogItem[]>([])
  const openId = ref<string | null>(null)

  const openItem = computed(() => items.value.find((item) => item.id === openId.value) ?? null)

  function find(id: string): BacklogItem | undefined {
    return items.value.find((item) => item.id === id)
  }

  function open(id: string): void {
    if (find(id)) openId.value = id
  }

  function close(): void {
    openId.value = null
  }

  function moveTo(id: string, column: BacklogColumn): void {
    const item = find(id)
    if (!item || !MOVABLE_COLUMNS.includes(column) || column === (item.column ?? DEFAULT_COLUMN)) return
    item.column = column
  }

  function toggleTag(id: string, tag: BacklogTag): void {
    const item = find(id)
    if (!item) return
    item.tags = item.tags.includes(tag) ? item.tags.filter((current) => current !== tag) : [...item.tags, tag]
  }

  function answer(itemId: string, questionId: string, option: string): void {
    const question = find(itemId)?.questions?.find((current) => current.id === questionId)
    if (question) question.selected = option
  }

  function toggleCriterion(itemId: string, criterionId: string): void {
    const criterion = find(itemId)?.criteria?.find((current) => current.id === criterionId)
    if (criterion) criterion.done = !criterion.done
  }

  return { items, openId, openItem, open, close, moveTo, toggleTag, answer, toggleCriterion }
})
