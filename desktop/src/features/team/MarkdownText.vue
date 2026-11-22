<script setup lang="ts">
import { computed } from 'vue'
import { agentIconSvg } from './agentArt'
import { renderMarkdown, type MentionAgent } from './markdown'
import { useRequests } from './useRequests'
import { useRoster } from './useRoster'

const MENTION_ICON_SIZE = 14
const MENTION_SELECTOR = '[data-mention]'
const CHECK_INDEX_ATTRIBUTE = 'checkIndex'

const props = defineProps<{ text: string; tasks?: boolean }>()
const emit = defineEmits<{ toggle: [index: number, done: boolean] }>()

const roster = useRoster()
const requests = useRequests()

function insertMention(event: MouseEvent): void {
  const element = event.target instanceof Element ? event.target.closest<HTMLElement>(MENTION_SELECTOR) : null
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

// renderMarkdown escapa todo o texto antes de montar o HTML, então v-html recebe só marcação nossa.
const html = computed(() => renderMarkdown(props.text, agents.value, { taskList: props.tasks }))
</script>

<template>
  <div
    class="wrap-break-word flex flex-col gap-2 text-control text-ink [&_a]:text-accent [&_a]:underline [&_blockquote]:border-l-2 [&_blockquote]:border-line [&_blockquote]:pl-3 [&_blockquote]:text-muted [&_code]:rounded-sm [&_code]:bg-surface-2 [&_code]:px-1 [&_code]:py-0.5 [&_code]:font-mono [&_code]:text-small [&_del]:text-muted [&_em]:italic [&_h3]:font-semibold [&_h4]:font-semibold [&_h5]:font-semibold [&_hr]:border-line [&_ol]:list-decimal [&_ol]:pl-5 [&_pre]:overflow-x-auto [&_pre]:rounded-md [&_pre]:border [&_pre]:border-line [&_pre]:bg-surface [&_pre]:p-3 [&_pre_code]:bg-transparent [&_pre_code]:p-0 [&_strong]:font-semibold [&_ul]:list-disc [&_ul]:pl-5"
    @click="insertMention"
    @change="toggleTask"
    v-html="html"
  />
</template>
