<script setup lang="ts">
import { computed } from 'vue'
import { agentName } from './agents'
import QueuedEntry from './QueuedEntry.vue'
import { useFeed } from './useFeed'
import { useQueueItems } from './useQueueItems'

const PANEL_CLASSES =
  'flex min-h-0 min-w-0 flex-none max-h-72 flex-col gap-3 overflow-y-auto rounded-dialog border border-line bg-surface p-4 transition-[flex] duration-300 ease-in-out motion-reduce:transition-none lg:max-h-none lg:flex-[1_1_240px]'
const TEAM_TITLE = 'Fila do time'
const AGENT_TITLE_PREFIX = 'Fila de '

const feed = useFeed()
const items = useQueueItems()

const title = computed(() => (feed.screen.kind === 'agent' ? AGENT_TITLE_PREFIX + agentName(feed.screen.agent) : TEAM_TITLE))
</script>

<template>
  <section :aria-label="title" :class="PANEL_CLASSES">
    <h2 class="text-lead text-ink">{{ title }}</h2>
    <ol class="flex flex-col gap-3">
      <li v-for="request in items" :key="request.id">
        <QueuedEntry :request="request" />
      </li>
    </ol>
  </section>
</template>
