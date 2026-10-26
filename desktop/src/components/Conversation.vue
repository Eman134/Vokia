<script setup lang="ts">
import { AGENTS, useTeam, type AgentProfile } from '../stores/useTeam'
import ThemeSwitch from './ThemeSwitch.vue'
import type { Speaker, VokiaMessage } from '../types/events'

const SPEAKER_NAMES: Record<Speaker, string> = {
  user: 'Você',
  iara: 'Iara',
  davi: 'Davi',
}

const USER_INITIAL = 'V'

const TIME_FORMAT = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' })

const team = useTeam()

function author(message: VokiaMessage): string {
  const betweenAgents = message.from !== 'user' && message.to !== 'user'
  return betweenAgents
    ? `${SPEAKER_NAMES[message.from]} → ${SPEAKER_NAMES[message.to]}`
    : SPEAKER_NAMES[message.from]
}

function profile(speaker: Speaker): AgentProfile | undefined {
  return AGENTS.find((agent) => agent.id === speaker)
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
      <li v-for="(entry, index) in team.feed" :key="index">
        <div v-if="entry.type === 'message' && entry.from === 'user'" class="flex items-start justify-end gap-2.5">
          <p
            class="wrap-break-word max-w-3/4 whitespace-pre-wrap rounded-card rounded-tr-sm bg-surface-2 px-4 py-3 text-control text-ink"
          >
            {{ entry.text }}
          </p>
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
        <div
          v-else
          class="flex items-center gap-2.5 rounded-card border border-danger bg-surface px-4 py-3"
        >
          <span class="size-1.75 shrink-0 rounded-full bg-danger" />
          <p class="wrap-break-word min-w-0 whitespace-pre-wrap text-control text-ink">{{ entry.message }}</p>
        </div>
      </li>
    </ol>
  </section>
</template>
