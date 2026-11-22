<script setup lang="ts">
import { computed } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import type { Agent } from '../team/events'
import { useRoster } from '../team/useRoster'
import CountStepper from './CountStepper.vue'
import { VIEW_COPY } from './orchestrationLabels'
import {
  INSTANCES_MAX,
  INSTANCES_MIN,
  INSTRUCTIONS_MAX_CHARS,
  TEXTAREA_CLASSES,
  WORKTREES_MAX,
  WORKTREES_MIN,
} from './orchestrationForm'
import type { FlowNodeConfig } from './useFlowDraft'

const PORTRAIT_SIZE = 36

const props = defineProps<{ agent: Agent; isLeader: boolean; config: FlowNodeConfig }>()
const emit = defineEmits<{ close: []; update: [patch: Partial<FlowNodeConfig>]; setLeader: []; remove: [] }>()

const roster = useRoster()
const instructionsCount = computed(() => `${props.config.instructions.length} / ${INSTRUCTIONS_MAX_CHARS}`)
</script>

<template>
  <section class="flex min-w-0 flex-col gap-4 rounded-md border border-line bg-surface p-4.5">
    <header class="flex items-center justify-between gap-3">
      <div class="flex min-w-0 items-center gap-2.5">
        <AgentPortrait :who="agent" :size="PORTRAIT_SIZE" />
        <div class="flex min-w-0 flex-col">
          <h3 class="truncate text-control font-bold text-ink">{{ roster.nameOf(agent) }}</h3>
          <span v-if="isLeader" class="text-small text-muted">{{ VIEW_COPY.leaderBadge }}</span>
        </div>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <UiButton variant="quiet" size="sm" :aria-label="VIEW_COPY.removeAgent" :title="VIEW_COPY.removeAgent" @press="emit('remove')">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14" />
          </svg>
        </UiButton>
        <UiButton variant="quiet" size="sm" @press="emit('close')">{{ VIEW_COPY.close }}</UiButton>
      </div>
    </header>

    <UiButton v-if="!isLeader" variant="primary" size="lg" @press="emit('setLeader')">
      {{ VIEW_COPY.setLeader }}
    </UiButton>

    <label class="flex flex-col gap-1.5">
      <span class="flex items-baseline justify-between gap-3 text-label font-medium text-muted">
        {{ VIEW_COPY.instructions }}
        <span class="font-mono text-tiny font-normal text-faint">{{ instructionsCount }}</span>
      </span>
      <textarea
        :value="config.instructions"
        rows="4"
        :class="TEXTAREA_CLASSES"
        @input="emit('update', { instructions: ($event.target as HTMLTextAreaElement).value })"
      />
    </label>
    <label class="flex flex-col gap-1.5">
      <span class="text-label font-medium text-muted">{{ VIEW_COPY.criteria }}</span>
      <textarea
        :value="config.criteria"
        rows="3"
        :class="TEXTAREA_CLASSES"
        @input="emit('update', { criteria: ($event.target as HTMLTextAreaElement).value })"
      />
    </label>
    <div class="flex flex-col gap-1.5">
      <span class="text-label font-medium text-muted">{{ VIEW_COPY.instances }}</span>
      <CountStepper
        :model-value="config.instances"
        :label="VIEW_COPY.instances"
        :min="INSTANCES_MIN"
        :max="INSTANCES_MAX"
        @update:model-value="emit('update', { instances: $event })"
      />
    </div>
    <div v-if="isLeader" class="flex flex-col gap-1.5">
      <span class="text-label font-medium text-muted">{{ VIEW_COPY.worktreesMax }}</span>
      <CountStepper
        :model-value="config.worktrees"
        :label="VIEW_COPY.worktreesMax"
        :min="WORKTREES_MIN"
        :max="WORKTREES_MAX"
        @update:model-value="emit('update', { worktrees: $event })"
      />
    </div>
  </section>
</template>
