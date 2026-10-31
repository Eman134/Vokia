<script setup lang="ts">
import AgentPortrait from './AgentPortrait.vue'
import { PORTRAIT_SIZES } from './portraitArt'
import type { AgentProfile } from './useTeam'

defineProps<{
  id: string
  agents: readonly AgentProfile[]
  activeIndex: number
  left: number
}>()

const emit = defineEmits<{
  choose: [agent: AgentProfile]
}>()
</script>

<template>
  <div
    class="absolute bottom-full mb-2 w-65 rounded-xl border border-line bg-surface p-1.25 shadow-float"
    :style="{ left: `${left}px` }"
  >
    <ul v-if="agents.length > 0" :id="id" role="listbox" aria-label="Agentes" class="flex flex-col gap-px">
      <li
        v-for="(agent, index) in agents"
        :id="`${id}-${agent.id}`"
        :key="agent.id"
        role="option"
        :aria-selected="index === activeIndex"
        class="flex cursor-pointer items-center gap-2.25 rounded-lg px-2 py-1.5"
        :class="index === activeIndex ? 'bg-accent-soft' : undefined"
        @mousedown.prevent
        @click="emit('choose', agent)"
      >
        <span class="flex shrink-0" aria-hidden="true">
          <AgentPortrait :who="agent.id" :size="PORTRAIT_SIZES.mention" />
        </span>
        <span class="text-label">
          <b class="font-medium text-ink">{{ agent.name }}</b>
          <span class="text-muted"> {{ agent.role }}</span>
        </span>
      </li>
    </ul>
    <p v-else class="px-2 py-1.5 text-caption text-muted">Nenhum agente</p>
  </div>
</template>
