<script setup lang="ts">
import { agentName, DEFAULT_RECIPIENT } from './agents'
import { isBetweenAgents, type FeedEntry } from './timeline'
import { useRoster } from './useRoster'
import type { AgentProfile } from './teamApi'
import type { PermissionOutcome, Speaker, VokiaMessage } from './events'
import ActivityRun from './ActivityRun.vue'
import MarkdownText from './MarkdownText.vue'
import AgentPortrait from './AgentPortrait.vue'
import { PORTRAIT_SIZES } from './portraitArt'
import UiButton from '../../ui/UiButton.vue'
import UserAvatar from '../profile/UserAvatar.vue'
import { useUserProfile } from '../profile/useUserProfile'
import { useBacklog } from '../backlog/useBacklog'
import { prefillFromMessage } from '../backlog/requestPrefill'

defineProps<{ entry: FeedEntry }>()

const roster = useRoster()
const backlog = useBacklog()
const userProfile = useUserProfile()

const META_AVATAR_SIZE = PORTRAIT_SIZES.stack
const TURN_INTO_REQUEST_LABEL = '+ virar pedido'
// Troca entre agentes só recolhe quando o texto ou a atividade passam destes limites.
const COLLAPSE_TEXT_CHARS = 600
const COLLAPSE_TOOL_COUNT = 5

const OUTCOME_LABELS: Record<PermissionOutcome, string> = {
  allowed: '✓ Aprovado:',
  denied: '✕ Negado:',
  expired: '⏱ Expirou sem resposta:',
}

const TIME_FORMAT = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' })

function speakerName(speaker: Speaker): string {
  return speaker === 'user' ? userProfile.displayName : agentName(speaker)
}

function author(message: VokiaMessage): string {
  return isBetweenAgents(message)
    ? `${speakerName(message.from)} → ${speakerName(message.to)}`
    : speakerName(message.from)
}

function profile(speaker: Speaker): AgentProfile | undefined {
  return roster.profileOf(speaker)
}

function isLong(message: VokiaMessage): boolean {
  return message.text.length > COLLAPSE_TEXT_CHARS || (message.activity?.tools.length ?? 0) > COLLAPSE_TOOL_COUNT
}

function turnIntoRequest(message: VokiaMessage): void {
  backlog.startRequest(prefillFromMessage({ text: message.text, from: message.from, time: message.time, author: author(message) }))
}

// Só o destinatário do agente é destacado como menção, como antes.
function highlightedNames(message: VokiaMessage): string[] {
  const recipient = message.to === 'user' ? undefined : profile(message.to)
  return recipient ? [recipient.name] : []
}
</script>

<template>
  <div v-if="entry.type === 'message' && entry.from === 'user'" class="ml-auto flex max-w-3/4 flex-col items-end gap-1.5">
    <span v-if="entry.to !== DEFAULT_RECIPIENT" class="text-caption text-muted">
      Para {{ speakerName(entry.to) }}
    </span>
    <div class="wrap-break-word rounded-card rounded-tr-tail bg-surface-2 px-4 py-3">
      <MarkdownText :text="entry.text" :mentions="highlightedNames(entry)" />
    </div>
    <div class="flex flex-wrap items-center justify-end gap-1.5 text-caption text-muted">
      <UserAvatar :size="META_AVATAR_SIZE" :title="userProfile.displayName" />
      <span class="font-semibold text-ink">{{ speakerName(entry.from) }}</span>
      <time>{{ TIME_FORMAT.format(entry.time) }}</time>
    </div>
  </div>
  <details
    v-else-if="entry.type === 'message' && isBetweenAgents(entry)"
    class="flex min-w-0 flex-col gap-1.5"
    :open="!isLong(entry)"
    data-testid="agent-exchange"
  >
    <summary class="flex min-w-0 cursor-pointer list-none flex-wrap items-center gap-x-1.5 gap-y-1 text-caption text-muted">
      <span class="flex items-center gap-1 font-semibold text-ink">
        <AgentPortrait :who="entry.from" :size="META_AVATAR_SIZE" />
        {{ speakerName(entry.from) }}
      </span>
      <span aria-hidden="true">→</span>
      <span class="flex items-center gap-1 font-semibold text-ink">
        <AgentPortrait :who="entry.to" :size="META_AVATAR_SIZE" />
        {{ speakerName(entry.to) }}
      </span>
      <time>{{ TIME_FORMAT.format(entry.time) }}</time>
    </summary>
    <ActivityRun v-if="entry.activity" :activity="entry.activity" />
    <MarkdownText :text="entry.text" />
    <div class="flex justify-end">
      <UiButton
        variant="quiet"
        size="sm"
        :disabled="entry.projectId === null"
        @press="turnIntoRequest(entry)"
      >
        {{ TURN_INTO_REQUEST_LABEL }}
      </UiButton>
    </div>
  </details>
  <div v-else-if="entry.type === 'message'" class="flex min-w-0 flex-col gap-1.5">
    <ActivityRun v-if="entry.activity" :activity="entry.activity" />
    <MarkdownText :text="entry.text" />
    <div class="flex flex-wrap items-center gap-1.5 text-caption text-muted">
      <span class="flex shrink-0" aria-hidden="true">
        <AgentPortrait :who="entry.from" :size="META_AVATAR_SIZE" />
      </span>
      <span class="font-semibold text-ink">{{ speakerName(entry.from) }}</span>
      <time>{{ profile(entry.from)?.role }} · {{ TIME_FORMAT.format(entry.time) }}</time>
      <UiButton
        class="ml-auto"
        variant="quiet"
        size="sm"
        :disabled="entry.projectId === null"
        @press="turnIntoRequest(entry)"
      >
        {{ TURN_INTO_REQUEST_LABEL }}
      </UiButton>
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
