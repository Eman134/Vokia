<script setup lang="ts">
import { agentName, DEFAULT_RECIPIENT } from './agents'
import { isBetweenAgents, type FeedEntry } from './timeline'
import { useRoster } from './useRoster'
import type { AgentProfile } from './teamApi'
import type { PermissionResultOutcome, Speaker, VokiaTaskResult } from './events'
import ActivityRun from './ActivityRun.vue'
import MarkdownText from './MarkdownText.vue'
import AgentPortrait from './AgentPortrait.vue'
import { PORTRAIT_SIZES } from './portraitArt'
import UserAvatar from '../profile/UserAvatar.vue'
import { useUserProfile } from '../profile/useUserProfile'
import { useBacklog } from '../backlog/useBacklog'
import UiButton from '../../ui/UiButton.vue'

defineProps<{ entry: FeedEntry }>()

const roster = useRoster()
const userProfile = useUserProfile()
const backlog = useBacklog()

const META_AVATAR_SIZE = PORTRAIT_SIZES.stack
const PROPOSAL_LABEL = 'Virar pedido'

const OUTCOME_LABELS: Record<PermissionResultOutcome, string> = {
  allowed: '✓ Aprovado:',
  denied: '✕ Negado:',
  expired: '⏱ Expirou sem resposta:',
  'project-rule': '✓ Regra do projeto:',
}

const TIME_FORMAT = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' })
const CHECK_ICON_PATH = 'M3.5 8.5l3 3 6-6.5'
const WARNING_ICON_PATH = 'M8 2.5L14 13.5H2Z M8 6.5v3M8 11.2v.3'

function taskResultTitle(entry: VokiaTaskResult): string {
  return entry.concluded ? `Tarefa concluída: ${entry.objective}` : `Tarefa interrompida: ${entry.objective}`
}

function taskResultDetail(entry: VokiaTaskResult): string {
  const turns = `${entry.turns} ${entry.turns === 1 ? 'turno' : 'turnos'}`
  const agents = entry.agents.map(agentName).join(', ')
  return [turns, agents, TIME_FORMAT.format(entry.time)].filter(Boolean).join(' · ')
}

function speakerName(speaker: Speaker): string {
  return speaker === 'user' ? userProfile.displayName : agentName(speaker)
}

function profile(speaker: Speaker): AgentProfile | undefined {
  return roster.profileOf(speaker)
}
</script>

<template>
  <div v-if="entry.type === 'message' && entry.from === 'user'" class="ml-auto flex max-w-3/4 flex-col items-end gap-1.5">
    <span v-if="entry.to !== DEFAULT_RECIPIENT" class="text-caption text-muted">
      Para {{ speakerName(entry.to) }}
    </span>
    <div class="wrap-break-word rounded-card rounded-tr-tail bg-surface-2 px-4 py-3">
      <MarkdownText :text="entry.text" />
    </div>
    <div class="flex flex-wrap items-center justify-end gap-1.5 text-caption text-muted">
      <UserAvatar :size="META_AVATAR_SIZE" :title="userProfile.displayName" />
      <span class="font-semibold text-ink">{{ speakerName(entry.from) }}</span>
      <time>{{ TIME_FORMAT.format(entry.time) }}</time>
    </div>
  </div>
  <div
    v-else-if="entry.type === 'message' && isBetweenAgents(entry)"
    class="flex min-w-0 flex-wrap items-start gap-x-2 gap-y-1"
    :title="TIME_FORMAT.format(entry.time)"
    data-testid="agent-exchange"
  >
    <span class="flex shrink-0 items-center gap-1.5 text-caption font-semibold text-ink">
      <AgentPortrait :who="entry.from" :size="META_AVATAR_SIZE" />
      {{ speakerName(entry.from) }}
    </span>
    <span aria-hidden="true" class="shrink-0 text-caption text-muted">→</span>
    <span class="flex shrink-0 items-center gap-1.5 text-caption font-semibold text-ink">
      <AgentPortrait :who="entry.to" :size="META_AVATAR_SIZE" />
      {{ speakerName(entry.to) }}
    </span>
    <div class="min-w-64 flex-1">
      <ActivityRun v-if="entry.activity" :activity="entry.activity" />
      <MarkdownText :text="entry.text" />
    </div>
  </div>
  <div v-else-if="entry.type === 'message'" class="flex min-w-0 flex-col gap-1.5">
    <ActivityRun v-if="entry.activity" :activity="entry.activity" />
    <MarkdownText :text="entry.text" />
    <UiButton
      v-if="entry.requestProposal"
      class="self-start"
      variant="quiet"
      size="sm"
      :disabled="entry.projectId === null || backlog.proposalCreated(entry)"
      @press="backlog.createFromProposal(entry)"
    >
      {{ PROPOSAL_LABEL }}
    </UiButton>
    <div class="flex flex-wrap items-center gap-1.5 text-caption text-muted">
      <span class="flex shrink-0" aria-hidden="true">
        <AgentPortrait :who="entry.from" :size="META_AVATAR_SIZE" />
      </span>
      <span class="font-semibold text-ink">{{ speakerName(entry.from) }}</span>
      <time>{{ profile(entry.from)?.role }} · {{ TIME_FORMAT.format(entry.time) }}</time>
    </div>
  </div>
  <p v-else-if="entry.type === 'notice'" class="text-caption text-muted">{{ entry.text }}</p>
  <div
    v-else-if="entry.type === 'task-result'"
    data-testid="task-result"
    class="flex items-start gap-2.5 rounded-card border border-line bg-surface px-4 py-3"
  >
    <svg
      class="mt-0.5 size-4 shrink-0"
      :class="entry.concluded ? 'text-ok' : 'text-warn-text'"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      <path :d="entry.concluded ? CHECK_ICON_PATH : WARNING_ICON_PATH" />
    </svg>
    <div class="flex min-w-0 flex-col gap-0.5">
      <p class="wrap-break-word text-control font-semibold text-ink">{{ taskResultTitle(entry) }}</p>
      <p class="wrap-break-word text-caption text-muted">{{ taskResultDetail(entry) }}</p>
    </div>
  </div>
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
