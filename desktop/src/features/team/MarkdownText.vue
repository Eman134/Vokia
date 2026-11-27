<script setup lang="ts">
import { computed } from 'vue'
import { agentIconSvg } from './agentArt'
import { renderMarkdown, type MentionAgent, type MentionTask } from './markdown'
import { useRequests } from './useRequests'
import { useRoster } from './useRoster'
import { useBacklog } from '../backlog/useBacklog'

const MENTION_ICON_SIZE = 14
const MENTION_SELECTOR = '[data-mention]'
const TASK_MENTION_SELECTOR = '[data-task-mention]'
const CHECK_INDEX_ATTRIBUTE = 'checkIndex'

const props = defineProps<{ text: string; tasks?: boolean }>()
const emit = defineEmits<{ toggle: [index: number, done: boolean] }>()

const roster = useRoster()
const requests = useRequests()
const backlog = useBacklog()

function insertMention(event: MouseEvent): void {
  const target = event.target instanceof Element ? event.target : null
  const taskElement = target?.closest<HTMLElement>(TASK_MENTION_SELECTOR)
  const itemId = taskElement?.dataset.taskMention
  if (itemId) {
    backlog.open(itemId)
    return
  }
  const element = target?.closest<HTMLElement>(MENTION_SELECTOR)
  const agent = element?.dataset.mention
  if (agent) requests.requestMention(agent)
}

function toggleTask(event: Event): void {
  const input = event.target
  if (!(input instanceof HTMLInputElement)) return
  const index = Number(input.dataset[CHECK_INDEX_ATTRIBUTE])
  if (Number.isInteger(index)) emit('toggle', index, input.checked)
}

const agents = computed<MentionAgent[]>(() =>
  roster.agents.map((agent) => ({ id: agent.id, name: agent.name, icon: agentIconSvg(agent.id, MENTION_ICON_SIZE) })),
)

const taskMentions = computed<MentionTask[]>(() =>
  backlog.items.map((item) => ({ id: item.id, publicId: item.publicId, title: item.title })),
)

// renderMarkdown escapa todo o texto antes de montar o HTML, então v-html recebe só marcação nossa.
const html = computed(() =>
  renderMarkdown(props.text, agents.value, { taskList: props.tasks, taskMentions: taskMentions.value }),
)
</script>

<template>
  <div
    class="wrap-break-word flex flex-col gap-2 text-control text-ink [&_a]:text-accent [&_a]:underline [&_blockquote]:border-l-2 [&_blockquote]:border-line [&_blockquote]:pl-3 [&_blockquote]:text-muted [&_code]:rounded-sm [&_code]:bg-surface-2 [&_code]:px-1 [&_code]:py-0.5 [&_code]:font-mono [&_code]:text-small [&_del]:text-muted [&_em]:italic [&_h3]:font-semibold [&_h4]:font-semibold [&_h5]:font-semibold [&_hr]:border-line [&_ol]:list-decimal [&_ol]:pl-5 [&_pre]:overflow-x-auto [&_pre]:rounded-md [&_pre]:border [&_pre]:border-line [&_pre]:bg-surface [&_pre]:p-3 [&_pre_code]:bg-transparent [&_pre_code]:p-0 [&_strong]:font-semibold [&_table]:w-full [&_table]:border-collapse [&_td]:border [&_td]:border-line [&_td]:px-3 [&_td]:py-1.5 [&_td]:align-top [&_th]:border [&_th]:border-line [&_th]:bg-surface-2 [&_th]:px-3 [&_th]:py-1.5 [&_th]:text-left [&_th]:font-semibold [&_ul]:list-disc [&_ul]:pl-5"
    @click="insertMention"
    @change="toggleTask"
    v-html="html"
  />
</template>
