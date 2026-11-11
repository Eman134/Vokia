<script setup lang="ts">
import { computed } from 'vue'
import { useAgentControl } from '../panel/useAgentControl'
import AgentPortrait from '../team/AgentPortrait.vue'
import { presenceOf } from '../team/agentStateStyle'
import type { AgentState } from '../team/events'
import type { AgentProfile } from '../team/teamApi'
import { useTeam } from '../team/useTeam'

const CARD_PORTRAIT_SIZE = 36

const TONE_CLASSES: Record<AgentState, string> = {
  waiting: 'border-2 border-warn bg-surface',
  thinking: 'bg-accent-soft',
  sending: 'bg-accent-soft',
  idle: 'border border-dashed border-line',
}

const props = defineProps<{ agent: AgentProfile }>()

const team = useTeam()
const control = useAgentControl()

const paused = computed(() => control.isPaused(props.agent.id))
const state = computed(() => team.stateOf(props.agent.id))
const presence = computed(() => presenceOf(state.value, paused.value))
const toneClass = computed(() => TONE_CLASSES[paused.value ? 'idle' : state.value])
</script>

<template>
  <div class="flex min-w-0 items-center gap-2.5 rounded-[16px] px-2.5 py-2" :class="toneClass">
    <AgentPortrait :who="agent.id" :size="CARD_PORTRAIT_SIZE" />
    <div class="flex min-w-0 flex-1 flex-col">
      <div class="flex min-w-0 items-center gap-1.5">
        <span class="truncate text-control font-semibold text-ink">{{ agent.name }}</span>
        <span class="size-1.75 shrink-0 rounded-full" :class="presence.dot" />
      </div>
      <span class="truncate text-small text-muted">{{ agent.role }} · {{ presence.label }}</span>
    </div>
  </div>
</template>
