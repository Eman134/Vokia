<script setup lang="ts">
import { computed } from 'vue'
import { agentName } from './agents'
import QueuedEntry from './QueuedEntry.vue'
import { useFeed } from './useFeed'
import { useRequests } from './useRequests'

const PANEL_CLASSES =
  'flex min-h-0 min-w-0 flex-col gap-3 overflow-y-auto rounded-dialog border border-line bg-surface p-4 transition-[flex] duration-300 ease-in-out motion-reduce:transition-none'
const EXPANDED_CLASSES = 'flex-none max-h-72 lg:max-h-none lg:flex-[1_1_240px]'
const COLLAPSED_CLASSES = 'flex-[0_0_9rem]'
const TEAM_TITLE = 'Fila do time'
const AGENT_TITLE_PREFIX = 'Fila de '
const EMPTY_TEXT = 'Nada na fila.'

const feed = useFeed()
const requests = useRequests()

const title = computed(() => (feed.screen.kind === 'agent' ? AGENT_TITLE_PREFIX + agentName(feed.screen.agent) : TEAM_TITLE))
const items = computed(() => (feed.screen.kind === 'agent' ? requests.queuedFor(feed.screen.agent) : requests.queue))
</script>

<template>
  <section :aria-label="title" :class="[PANEL_CLASSES, items.length > 0 ? EXPANDED_CLASSES : COLLAPSED_CLASSES]">
    <h2 class="text-lead text-ink">{{ title }}</h2>
    <ol v-if="items.length > 0" class="flex flex-col gap-3">
      <li v-for="request in items" :key="request.id">
        <QueuedEntry :request="request" />
      </li>
    </ol>
    <p v-else class="text-caption text-muted">{{ EMPTY_TEXT }}</p>
  </section>
</template>
