<script setup lang="ts">
import { AGENTS, useTeam } from './useTeam'
import ThemeSwitch from '../theme/ThemeSwitch.vue'
import MessageEntry from './MessageEntry.vue'

const AGENT_GROUP_TITLE = AGENTS.map((agent) => agent.name).join(' ↔ ')

const team = useTeam()

function countLabel(count: number): string {
  return count === 1 ? '1 mensagem' : `${count} mensagens`
}
</script>

<template>
  <section class="flex-1 overflow-y-auto px-8 pb-40 pt-6">
    <header class="mb-6 flex items-center justify-between gap-4">
      <h1 class="text-title text-ink">Conversas</h1>
      <ThemeSwitch />
    </header>
    <p v-if="team.feed.length === 0" class="text-control text-muted">Nenhuma mensagem ainda.</p>
    <ol class="flex max-w-3xl flex-col gap-3">
      <li v-for="item in team.timeline" :key="item.start">
        <details v-if="item.type === 'group'" class="rounded-card border border-line bg-surface px-4 py-3">
          <summary class="flex cursor-pointer list-none items-baseline gap-2">
            <span class="text-caption font-semibold text-ink">{{ AGENT_GROUP_TITLE }}</span>
            <span class="text-caption text-muted">{{ countLabel(item.messages.length) }}</span>
          </summary>
          <ol class="mt-3 flex flex-col gap-3">
            <li v-for="(message, index) in item.messages" :key="index">
              <MessageEntry :entry="message" />
            </li>
          </ol>
        </details>
        <MessageEntry v-else :entry="item.entry" />
      </li>
    </ol>
    <div
      role="status"
      :aria-label="team.thinkingAgent ? `${team.thinkingAgent.name} está pensando` : undefined"
      class="mt-3 flex max-w-3xl items-start gap-2.5"
    >
      <template v-if="team.thinkingAgent">
        <span
          class="grid size-7 shrink-0 place-items-center rounded-full border border-line bg-surface-2 text-small font-semibold text-ink"
        >
          {{ team.thinkingAgent.name.charAt(0) }}
        </span>
        <div class="min-w-0">
          <header class="flex items-baseline gap-2">
            <span class="text-caption font-semibold text-ink">{{ team.thinkingAgent.name }}</span>
            <span class="text-caption text-muted">pensando</span>
          </header>
          <div class="mt-1.5 flex items-center gap-1">
            <span class="size-1.5 rounded-full bg-muted motion-reduce:animate-none animate-dot-1" />
            <span class="size-1.5 rounded-full bg-muted motion-reduce:animate-none animate-dot-2" />
            <span class="size-1.5 rounded-full bg-muted motion-reduce:animate-none animate-dot-3" />
          </div>
        </div>
      </template>
    </div>
  </section>
</template>
