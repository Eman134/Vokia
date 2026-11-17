<script setup lang="ts">
import { computed, ref, useId } from 'vue'
import type { TurnActivity } from './events'
import { activitySentence, thinkingLabel, toolRowLabel } from './activitySummary'

const props = defineProps<{ activity: TurnActivity }>()

const open = ref(false)
const listId = useId()
const sentence = computed(() => activitySentence(props.activity.tools))
const rows = computed(() =>
  props.activity.tools.map((tool) => ({ id: tool.callId, failed: tool.status === 'failed', ...toolRowLabel(tool) })),
)
</script>

<template>
  <div class="mt-0.5 flex flex-col gap-1 text-caption text-muted" data-testid="activity-run">
    <span v-if="activity.thinkingMs !== undefined" data-testid="activity-thinking">{{ thinkingLabel(activity.thinkingMs) }}</span>
    <template v-if="rows.length > 0">
      <button
        type="button"
        class="flex w-fit items-center gap-1 text-left hover:text-ink focus:outline-2 focus:outline-solid focus:outline-accent"
        :aria-expanded="open"
        :aria-controls="listId"
        data-testid="activity-toggle"
        @click="open = !open"
      >
        <span aria-hidden="true">{{ open ? '▾' : '▸' }}</span>
        <span class="wrap-break-word min-w-0" data-testid="activity-sentence">{{ sentence }}</span>
      </button>
      <ul v-show="open" :id="listId" class="flex flex-col gap-0.5 pl-4">
        <li
          v-for="row in rows"
          :key="row.id"
          :title="row.title"
          :class="['wrap-break-word', row.failed ? 'text-danger' : '']"
          data-testid="activity-row"
        >
          {{ row.label }}
          <span v-if="row.changes" class="font-mono text-tiny">{{ row.changes }}</span>
        </li>
      </ul>
    </template>
  </div>
</template>
