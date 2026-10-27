<script setup lang="ts">
import type { AgentProfile } from './useTeam'

defineProps<{
  id: string
  agents: readonly AgentProfile[]
  activeIndex: number
}>()

const emit = defineEmits<{
  choose: [agent: AgentProfile]
}>()
</script>

<template>
  <div class="absolute bottom-full left-0 mb-2 w-60 rounded-popover border border-line bg-surface p-1.5 shadow-float">
    <ul v-if="agents.length > 0" :id="id" role="listbox" aria-label="Agentes" class="flex flex-col gap-0.5">
      <li
        v-for="(agent, index) in agents"
        :id="`${id}-${agent.id}`"
        :key="agent.id"
        role="option"
        :aria-selected="index === activeIndex"
        class="flex cursor-pointer items-center gap-2.5 rounded-control px-2 py-1.5"
        :class="index === activeIndex ? 'bg-surface-2' : undefined"
        @mousedown.prevent
        @click="emit('choose', agent)"
      >
        <span
          class="grid size-7 shrink-0 place-items-center rounded-full border border-line bg-surface-2 text-small font-semibold text-ink"
          aria-hidden="true"
        >
          {{ agent.name.charAt(0) }}
        </span>
        <span class="min-w-0">
          <span class="block text-label font-semibold text-ink">{{ agent.name }}</span>
          <span class="block text-caption text-muted">{{ agent.role }}</span>
        </span>
      </li>
    </ul>
    <p v-else class="px-2 py-1.5 text-caption text-muted">Nenhum agente</p>
  </div>
</template>
