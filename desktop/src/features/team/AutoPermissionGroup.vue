<script setup lang="ts">
import { computed, ref, useId } from 'vue'
import type { Agent, FeedPermissionResult } from './events'
import { agentName } from './agents'
import { autoPermissionSentence, permissionResultRow } from './permissionSummary'
import ActivityRow from './ActivityRow.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'

const props = defineProps<{ agent: Agent; results: FeedPermissionResult[] }>()

const LIST_CLASSES = 'flex min-w-0 flex-col gap-1 text-caption text-muted'

const open = ref(false)
const listId = useId()
const sentence = computed(() => autoPermissionSentence(agentName(props.agent), props.results))
const rows = computed(() => props.results.map(permissionResultRow))
</script>

<template>
  <ul v-if="rows.length === 1" :class="LIST_CLASSES" data-testid="permission-result">
    <ActivityRow :row="rows[0]" />
  </ul>
  <div v-else :class="LIST_CLASSES" data-testid="auto-permission-group">
    <button
      type="button"
      class="flex w-fit cursor-pointer items-center gap-1 text-left hover:text-ink"
      :class="INK_FOCUS_CLASSES"
      :aria-expanded="open"
      :aria-controls="listId"
      data-testid="auto-permission-toggle"
      @click="open = !open"
    >
      <span class="min-w-0 wrap-break-word">{{ sentence }}</span>
      <span aria-hidden="true">{{ open ? '▾' : '▸' }}</span>
    </button>
    <ul v-show="open" :id="listId" class="flex flex-col gap-1 pl-4">
      <ActivityRow v-for="row in rows" :key="row.id" :row="row" />
    </ul>
  </div>
</template>
