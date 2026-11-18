<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import { useAgentControl } from '../panel/useAgentControl'
import AgentPortrait from '../team/AgentPortrait.vue'
import { presenceOf } from '../team/agentStateStyle'
import type { BacklogItem } from '../backlog/backlogTypes'
import type { AgentState } from '../team/events'
import type { AgentProfile } from '../team/teamApi'
import { useFeed } from '../team/useFeed'
import {
  CONFIRM_NO_LABEL,
  CONFIRM_YES_LABEL,
  REPLACE_AGENT_LABEL,
  UNLINK_AGENT_LABEL,
  UNLINK_CONFIRM_TEXT,
} from './teamMapLayers'

const CARD_PORTRAIT_SIZE = 36
const ACTION_ICON_SIZE = 14

const TONE_CLASSES: Record<AgentState, string> = {
  waiting: 'border-2 border-warn bg-surface',
  thinking: 'bg-accent-soft',
  sending: 'bg-accent-soft',
  idle: 'border border-dashed border-line',
}

const props = defineProps<{ agent: AgentProfile; activeCard: BacklogItem | undefined }>()
const emit = defineEmits<{ replace: []; unlink: [] }>()

const feed = useFeed()
const control = useAgentControl()
const confirmingUnlink = ref(false)

const paused = computed(() => control.isPaused(props.agent.id))
const state = computed(() => feed.stateOf(props.agent.id))
const presence = computed(() => presenceOf(state.value, paused.value))
const toneClass = computed(() => TONE_CLASSES[paused.value ? 'idle' : state.value])

function confirmUnlink(): void {
  confirmingUnlink.value = false
  emit('unlink')
}
</script>

<template>
  <div class="relative flex min-w-0 items-center gap-2.5 rounded-[16px] px-2.5 py-2" :class="toneClass">
    <div class="absolute top-1 right-1 flex items-center gap-0.5">
      <template v-if="!confirmingUnlink">
        <UiButton variant="quiet" size="sm" :aria-label="REPLACE_AGENT_LABEL" @press="emit('replace')">
          <svg
            :width="ACTION_ICON_SIZE"
            :height="ACTION_ICON_SIZE"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M17 2l4 4-4 4" />
            <path d="M3 11v-1a4 4 0 0 1 4-4h14" />
            <path d="M7 22l-4-4 4-4" />
            <path d="M21 13v1a4 4 0 0 1-4 4H3" />
          </svg>
        </UiButton>
        <UiButton variant="quiet" size="sm" :aria-label="UNLINK_AGENT_LABEL" @press="confirmingUnlink = true">
          <svg
            :width="ACTION_ICON_SIZE"
            :height="ACTION_ICON_SIZE"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M18 6 6 18M6 6l12 12" />
          </svg>
        </UiButton>
      </template>
      <div v-else class="flex items-center gap-1 rounded-control bg-surface px-1.5 py-0.5 shadow-seg">
        <span class="text-small text-ink">{{ UNLINK_CONFIRM_TEXT }}</span>
        <UiButton variant="danger" size="sm" @press="confirmUnlink()">{{ CONFIRM_YES_LABEL }}</UiButton>
        <UiButton variant="quiet" size="sm" @press="confirmingUnlink = false">{{ CONFIRM_NO_LABEL }}</UiButton>
      </div>
    </div>
    <AgentPortrait :who="agent.id" :size="CARD_PORTRAIT_SIZE" />
    <div class="flex min-w-0 flex-1 flex-col">
      <div class="flex min-w-0 items-center gap-1.5">
        <span class="truncate text-control font-semibold text-ink">{{ agent.name }}</span>
        <span class="size-1.75 shrink-0 rounded-full" :class="presence.dot" />
      </div>
      <span v-if="activeCard" class="truncate text-small text-muted">{{ activeCard.id }} · {{ activeCard.title }}</span>
      <span v-else class="truncate text-small text-muted">{{ agent.role }} · {{ presence.label }}</span>
    </div>
  </div>
</template>
