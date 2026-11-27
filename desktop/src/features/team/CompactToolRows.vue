<script setup lang="ts">
import { computed, ref, useId } from 'vue'
import type { ToolActivity } from './events'
import {
  activityRows,
  activitySentence,
  type ActivityRowModel,
  type AgentNameOf,
  type CardTitleOf,
} from './activitySummary'
import ActivityRow from './ActivityRow.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'

const COMPACT_THRESHOLD = 3
const MAX_TRAILING_RUNNING = 2

const props = defineProps<{
  tools: readonly ToolActivity[]
  rows: readonly ActivityRowModel[]
  cardTitleOf: CardTitleOf
  agentNameOf: AgentNameOf
}>()

const expanded = ref(false)
const listId = useId()

const split = computed(() => {
  if (props.rows.length < COMPACT_THRESHOLD) {
    return { compactedRows: [] as ActivityRowModel[], summary: '', activeRows: props.rows }
  }
  let trailing = 0
  for (let i = props.tools.length - 1; i >= 0 && trailing < MAX_TRAILING_RUNNING; i -= 1) {
    if (props.tools[i]?.status === 'running') {
      trailing += 1
    } else {
      break
    }
  }
  const compactedTools = trailing > 0 ? props.tools.slice(0, props.tools.length - trailing) : props.tools
  if (compactedTools.length >= COMPACT_THRESHOLD) {
    return {
      compactedRows: activityRows(compactedTools, props.cardTitleOf, props.agentNameOf),
      summary: activitySentence(compactedTools),
      activeRows: trailing > 0 ? activityRows(props.tools.slice(-trailing), props.cardTitleOf, props.agentNameOf) : [],
    }
  }
  return {
    compactedRows: props.rows,
    summary: activitySentence(props.tools),
    activeRows: [] as ActivityRowModel[],
  }
})
</script>

<template>
  <template v-if="split.compactedRows.length > 0">
    <li class="list-none">
      <button
        type="button"
        class="flex w-fit max-w-full min-w-0 cursor-pointer items-center gap-1 text-left hover:text-ink"
        :class="INK_FOCUS_CLASSES"
        :aria-expanded="expanded"
        :aria-controls="listId"
        data-testid="activity-live-compact-toggle"
        @click="expanded = !expanded"
      >
        <span class="min-w-0 wrap-break-word">{{ split.summary }}</span>
        <span aria-hidden="true">{{ expanded ? '▾' : '▸' }}</span>
      </button>
      <ul v-show="expanded" :id="listId" class="mt-1 flex min-w-0 flex-col gap-1 pl-4">
        <ActivityRow v-for="row in split.compactedRows" :key="row.id" :row="row" />
      </ul>
    </li>
  </template>
  <ActivityRow v-for="row in split.activeRows" :key="row.id" :row="row" />
</template>
