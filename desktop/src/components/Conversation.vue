<script setup lang="ts">
import { useTeam } from '../stores/useTeam'
import type { Speaker, VokiaMessage } from '../types/events'

const SPEAKER_NAMES: Record<Speaker, string> = {
  user: 'Você',
  iara: 'Iara',
  davi: 'Davi',
}

const TIME_FORMAT = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' })

const team = useTeam()

function author(message: VokiaMessage): string {
  const betweenAgents = message.from !== 'user' && message.to !== 'user'
  return betweenAgents
    ? `${SPEAKER_NAMES[message.from]} → ${SPEAKER_NAMES[message.to]}`
    : SPEAKER_NAMES[message.from]
}
</script>

<template>
  <section class="flex-1 overflow-y-auto px-8 pb-40 pt-6">
    <h1 class="mb-6 text-lg font-semibold text-ink">Conversas</h1>
    <p v-if="team.feed.length === 0" class="text-sm text-ink/50">Nenhuma mensagem ainda.</p>
    <ol class="flex max-w-3xl flex-col gap-3">
      <li v-for="(entry, index) in team.feed" :key="index">
        <article
          v-if="entry.type === 'message'"
          class="rounded-2xl border border-line bg-white px-4 py-3"
        >
          <header class="mb-1 flex items-baseline gap-2">
            <span class="text-sm font-medium" :class="entry.from === 'user' ? 'text-ink' : 'text-accent'">
              {{ author(entry) }}
            </span>
            <time class="text-xs text-ink/50">{{ TIME_FORMAT.format(entry.time) }}</time>
          </header>
          <p class="whitespace-pre-wrap text-sm text-ink">{{ entry.text }}</p>
        </article>
        <p v-else class="rounded-2xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
          {{ entry.message }}
        </p>
      </li>
    </ol>
  </section>
</template>
