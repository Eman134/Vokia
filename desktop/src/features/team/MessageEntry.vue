<script setup lang="ts">
import { AGENTS, DEFAULT_RECIPIENT, isBetweenAgents, type AgentProfile, type FeedEntry } from './useTeam'
import { mentionSegments, mentionSpans, type MentionSegment } from './composerMentions'
import type { Speaker, VokiaMessage } from './events'

defineProps<{ entry: FeedEntry }>()

const SPEAKER_NAMES: Record<Speaker, string> = {
  user: 'Você',
  iara: 'Iara',
  davi: 'Davi',
}

const USER_INITIAL = 'V'

const TIME_FORMAT = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' })

function author(message: VokiaMessage): string {
  return isBetweenAgents(message)
    ? `${SPEAKER_NAMES[message.from]} → ${SPEAKER_NAMES[message.to]}`
    : SPEAKER_NAMES[message.from]
}

function profile(speaker: Speaker): AgentProfile | undefined {
  return AGENTS.find((agent) => agent.id === speaker)
}

function highlightedSegments(message: VokiaMessage): MentionSegment[] {
  const recipient = message.to === 'user' ? [] : [message.to]
  return mentionSegments(message.text, mentionSpans(message.text, new Set(recipient)))
}
</script>

<template>
  <div v-if="entry.type === 'message' && entry.from === 'user'" class="flex items-start justify-end gap-2.5">
    <div class="flex max-w-3/4 flex-col items-end gap-1">
      <span v-if="entry.to !== DEFAULT_RECIPIENT" class="text-caption text-muted">
        Para {{ SPEAKER_NAMES[entry.to] }}
      </span>
      <p
        class="wrap-break-word whitespace-pre-wrap rounded-card rounded-tr-tail bg-surface-2 px-4 py-3 text-control text-ink"
      >
        <span
          v-for="(segment, index) in highlightedSegments(entry)"
          :key="index"
          :class="segment.agent === null ? undefined : 'text-accent'"
          >{{ segment.text }}</span
        >
      </p>
    </div>
    <span class="grid size-7 shrink-0 place-items-center rounded-full bg-ink text-small font-semibold text-bg">
      {{ USER_INITIAL }}
    </span>
  </div>
  <div v-else-if="entry.type === 'message'" class="flex items-start gap-2.5">
    <span
      class="grid size-7 shrink-0 place-items-center rounded-full border border-line bg-surface-2 text-small font-semibold text-ink"
    >
      {{ profile(entry.from)?.name.charAt(0) }}
    </span>
    <div class="min-w-0">
      <header class="flex items-baseline gap-2">
        <span class="text-caption font-semibold text-ink">{{ author(entry) }}</span>
        <time class="text-caption text-muted">
          {{ profile(entry.from)?.role }} · {{ TIME_FORMAT.format(entry.time) }}
        </time>
      </header>
      <p class="wrap-break-word mt-0.5 whitespace-pre-wrap text-control text-ink">{{ entry.text }}</p>
    </div>
  </div>
  <div v-else class="flex items-center gap-2.5 rounded-card border border-danger bg-surface px-4 py-3">
    <span class="size-1.75 shrink-0 rounded-full bg-danger" />
    <p class="wrap-break-word min-w-0 whitespace-pre-wrap text-control text-ink">{{ entry.message }}</p>
  </div>
</template>
