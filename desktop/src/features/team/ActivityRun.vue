<script setup lang="ts">
import { computed, ref, useId } from 'vue'
import type { TurnActivity } from './events'
import { activityRows, activitySentence, thinkingRow, type AgentNameOf, type CardTitleOf } from './activitySummary'
import ActivityRow from './ActivityRow.vue'
import MarkdownText from './MarkdownText.vue'
import { useBacklog } from '../backlog/useBacklog'
import { useRoster } from './useRoster'

import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
const props = defineProps<{ activity: TurnActivity; live?: boolean }>()

const backlog = useBacklog()
const roster = useRoster()
const open = ref(false)
const listId = useId()
const thinkingOpen = ref(false)
const thinkingListId = useId()
const sentence = computed(() => activitySentence(props.activity.tools))
const cardTitleOf: CardTitleOf = (cardId) => backlog.items.find((item) => item.id === cardId)?.title
const agentNameOf: AgentNameOf = (id) => roster.profileOf(id)?.name
const rows = computed(() => activityRows(props.activity.tools, cardTitleOf, agentNameOf))
</script>

<template>
  <div class="mt-0.5 flex flex-col gap-1 text-caption text-muted" data-testid="activity-run">
    <div v-if="!live && activity.thinkingMs !== undefined" class="flex flex-col gap-1" data-testid="activity-thinking">
      <ul class="flex flex-col">
        <ActivityRow :row="thinkingRow(activity.thinkingMs)" />
      </ul>
      <template v-if="activity.thinkingText">
        <button
          type="button"
          class="flex w-fit cursor-pointer items-center gap-1 pl-5 text-left hover:text-ink" :class="INK_FOCUS_CLASSES"
          :aria-expanded="thinkingOpen"
          :aria-controls="thinkingListId"
          data-testid="activity-thinking-toggle"
          @click="thinkingOpen = !thinkingOpen"
        >
          <span>Raciocínio</span>
          <span aria-hidden="true">{{ thinkingOpen ? '▾' : '▸' }}</span>
        </button>
        <div v-if="thinkingOpen" :id="thinkingListId" class="min-w-0 pl-5" data-testid="activity-thinking-text">
          <MarkdownText :text="activity.thinkingText" />
        </div>
      </template>
    </div>
    <ul v-if="live && rows.length > 0" class="flex flex-col gap-1" data-testid="activity-live">
      <ActivityRow v-for="row in rows" :key="row.id" :row="row" />
    </ul>
    <template v-else-if="rows.length > 0">
      <button
        type="button"
        class="flex w-fit cursor-pointer items-center gap-1 text-left hover:text-ink" :class="INK_FOCUS_CLASSES"
        :aria-expanded="open"
        :aria-controls="listId"
        data-testid="activity-toggle"
        @click="open = !open"
      >
        <span class="wrap-break-word min-w-0" data-testid="activity-sentence">{{ sentence }}</span>
        <span aria-hidden="true">{{ open ? '▾' : '▸' }}</span>
      </button>
      <ul v-show="open" :id="listId" class="flex flex-col gap-1 pl-4">
        <ActivityRow v-for="row in rows" :key="row.id" :row="row" />
      </ul>
    </template>
  </div>
</template>
