<script setup lang="ts">
import { computed, ref, useId } from 'vue'
import type { TurnActivity } from './events'
import { activityRows, activitySentence, thinkingLabel, type CardTitleOf } from './activitySummary'
import ActivityRow from './ActivityRow.vue'
import { useBacklog } from '../backlog/useBacklog'

const props = defineProps<{ activity: TurnActivity; live?: boolean }>()

const backlog = useBacklog()
const open = ref(false)
const listId = useId()
const sentence = computed(() => activitySentence(props.activity.tools))
const cardTitleOf: CardTitleOf = (cardId) => backlog.items.find((item) => item.id === cardId)?.title
const rows = computed(() => activityRows(props.activity.tools, cardTitleOf))
</script>

<template>
  <div class="mt-0.5 flex flex-col gap-1 text-caption text-muted" data-testid="activity-run">
    <span v-if="!live && activity.thinkingMs !== undefined" data-testid="activity-thinking">{{ thinkingLabel(activity.thinkingMs) }}</span>
    <ul v-if="live && rows.length > 0" class="flex flex-col gap-1" data-testid="activity-live">
      <ActivityRow v-for="row in rows" :key="row.id" :row="row" />
    </ul>
    <template v-else-if="rows.length > 0">
      <button
        type="button"
        class="flex w-fit cursor-pointer items-center gap-1 text-left hover:text-ink focus:outline-2 focus:outline-solid focus:outline-accent"
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
