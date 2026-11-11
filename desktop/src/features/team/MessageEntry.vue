<script setup lang="ts">
import { agentName, DEFAULT_RECIPIENT, isBetweenAgents, type FeedEntry } from './useTeam'
import { useRoster } from './useRoster'
import type { AgentProfile } from './teamApi'
import type { PermissionOutcome, Speaker, VokiaMessage } from './events'
import MarkdownText from './MarkdownText.vue'

defineProps<{ entry: FeedEntry }>()

const roster = useRoster()

const USER_NAME = 'Você'

const OUTCOME_LABELS: Record<PermissionOutcome, string> = {
  allowed: '✓ Aprovado:',
  denied: '✕ Negado:',
  expired: '⏱ Expirou sem resposta:',
}

const USER_INITIAL = 'V'

const TIME_FORMAT = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' })

function speakerName(speaker: Speaker): string {
  return speaker === 'user' ? USER_NAME : agentName(speaker)
}

function author(message: VokiaMessage): string {
  return isBetweenAgents(message)
    ? `${speakerName(message.from)} → ${speakerName(message.to)}`
    : speakerName(message.from)
}

function profile(speaker: Speaker): AgentProfile | undefined {
  return roster.profileOf(speaker)
}

// Só o destinatário do agente é destacado como menção, como antes.
function highlightedNames(message: VokiaMessage): string[] {
  const recipient = message.to === 'user' ? undefined : profile(message.to)
  return recipient ? [recipient.name] : []
}
</script>

<template>
  <div v-if="entry.type === 'message' && entry.from === 'user'" class="flex items-start justify-end gap-2.5">
    <div class="flex max-w-3/4 flex-col items-end gap-1">
      <span v-if="entry.to !== DEFAULT_RECIPIENT" class="text-caption text-muted">
        Para {{ speakerName(entry.to) }}
      </span>
      <div class="wrap-break-word rounded-card rounded-tr-tail bg-surface-2 px-4 py-3">
        <MarkdownText :text="entry.text" :mentions="highlightedNames(entry)" />
      </div>
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
      <div class="mt-0.5">
        <MarkdownText :text="entry.text" />
      </div>
    </div>
  </div>
  <p v-else-if="entry.type === 'notice'" class="text-caption text-muted">{{ entry.text }}</p>
  <p
    v-else-if="entry.type === 'permission-result'"
    data-testid="permission-result"
    class="wrap-break-word whitespace-pre-wrap text-caption text-muted"
  >
    {{ OUTCOME_LABELS[entry.outcome] }} {{ entry.detail }}
  </p>
  <div v-else class="flex items-center gap-2.5 rounded-card border border-danger bg-surface px-4 py-3">
    <span class="size-1.75 shrink-0 rounded-full bg-danger" />
    <p class="wrap-break-word min-w-0 whitespace-pre-wrap text-control text-ink">{{ entry.message }}</p>
  </div>
</template>
