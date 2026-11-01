<script setup lang="ts">
import { ref } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import UiField from '../../ui/UiField.vue'
import { useDecisions } from '../decisions/useDecisions'
import { presenceOf } from '../team/agentStateStyle'
import AgentProjectIcons from '../team/AgentProjectIcons.vue'
import type { Agent } from '../team/events'
import { AGENTS, useTeam } from '../team/useTeam'
import {
  BADGE_BY_STATUS,
  DONE_TODAY_LABEL,
  elapsedLabel,
  messageFieldLabel,
  NO_RECENT_TEXT,
  NO_TASK_TEXT,
  NOW_TITLE,
  OPEN_CHAT_LABEL,
  PAUSE_LABEL,
  QUEUE_TITLE,
  RECENT_TITLE,
  RESUME_LABEL,
  YOU_LABEL,
} from './nowCopy'
import { useAgentActivity } from './useAgentActivity'
import { useAgentControl } from './useAgentControl'

const props = defineProps<{ agent: Agent }>()

const team = useTeam()
const decisions = useDecisions()
const control = useAgentControl()
const { task, since, recent, queue, doneToday, status, step } = useAgentActivity(props.agent)
const profile = AGENTS.find((candidate) => candidate.id === props.agent)!
const message = ref('')

function formatTime(time: number): string {
  return new Date(time).toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' })
}

function openChat(): void {
  decisions.setOpen(false)
  team.requestMention(props.agent)
}

async function submit(): Promise<void> {
  const text = message.value.trim()
  if (text !== '' && (await team.send(text, props.agent))) {
    message.value = ''
  }
}
</script>

<template>
  <div class="flex max-h-96 flex-col gap-3 overflow-y-auto" :data-testid="`agent-now-${agent}`">
    <header class="flex flex-wrap items-center gap-2">
      <span
        class="size-1.75 rounded-full"
        :class="presenceOf(team.states[agent], control.isPaused(agent)).dot"
        aria-hidden="true"
      />
      <p class="text-label font-semibold text-ink">{{ profile.name }} · {{ profile.role }}</p>
      <AgentProjectIcons :agent="agent" />
      <UiBadge :status="BADGE_BY_STATUS[status]" :label="status" data-testid="agent-now-status" />
      <span v-if="since !== null" class="text-caption text-muted">{{ elapsedLabel(since, Date.now()) }}</span>
      <span class="ml-auto flex gap-1">
        <UiButton variant="quiet" size="sm" data-testid="agent-now-pause" @press="team.togglePause(agent)">
          {{ control.isPaused(agent) ? RESUME_LABEL : PAUSE_LABEL }}
        </UiButton>
        <UiButton variant="quiet" size="sm" @press="openChat">{{ OPEN_CHAT_LABEL }}</UiButton>
      </span>
    </header>
    <section :aria-label="NOW_TITLE">
      <p class="text-caption text-muted">{{ NOW_TITLE }}</p>
      <p class="wrap-break-word line-clamp-3 text-label text-ink">{{ task ?? NO_TASK_TEXT }}</p>
      <p class="text-caption text-muted" data-testid="agent-now-step">{{ step }}</p>
    </section>
    <section :aria-label="RECENT_TITLE">
      <p class="text-caption text-muted">{{ RECENT_TITLE }}</p>
      <p v-if="recent.length === 0" class="text-label text-faint">{{ NO_RECENT_TEXT }}</p>
      <ul v-else class="flex flex-col gap-0.5">
        <li v-for="(action, index) in recent" :key="index" class="flex gap-2 text-label">
          <span class="shrink-0 text-faint">{{ formatTime(action.time) }}</span>
          <span class="wrap-break-word min-w-0 text-ink">{{ action.text }}</span>
        </li>
      </ul>
    </section>
    <section v-if="queue.length > 0" :aria-label="QUEUE_TITLE">
      <p class="text-caption text-muted">{{ QUEUE_TITLE }}</p>
      <ol class="flex flex-col gap-0.5">
        <li v-for="(request, index) in queue" :key="request.id" class="flex gap-2 text-label" data-testid="agent-now-queued">
          <span class="shrink-0 text-faint">{{ index + 1 }}</span>
          <span class="wrap-break-word min-w-0 text-ink">{{ request.text }}</span>
          <span class="shrink-0 text-muted">{{ YOU_LABEL }}</span>
        </li>
      </ol>
    </section>
    <p class="text-caption text-muted">
      {{ DONE_TODAY_LABEL }} <span class="font-semibold text-ink" data-testid="agent-now-done">{{ doneToday }}</span>
    </p>
    <form @submit.prevent="submit">
      <UiField v-model="message" :label="messageFieldLabel(profile.name)" />
    </form>
  </div>
</template>
