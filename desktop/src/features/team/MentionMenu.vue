<script setup lang="ts">
import AgentPortrait from './AgentPortrait.vue'
import { PORTRAIT_SIZES } from './portraitArt'
import type { AgentProfile } from './teamApi'

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
    class="absolute bottom-full mb-2 w-65 rounded-md border border-line bg-surface p-1.5 shadow-paper"
    :style="{ left: `${left}px` }"
  >
    <ul v-if="agents.length > 0" :id="id" role="listbox" aria-label="Agentes" class="flex flex-col gap-px">
      <li
        v-for="(agent, index) in agents"
        :id="`${id}-${agent.id}`"
        :key="agent.id"
        role="option"
        :aria-selected="index === activeIndex"
        class="flex cursor-pointer items-center gap-2.5 rounded-md px-2 py-1.5"
        :class="index === activeIndex ? 'bg-accent-soft' : undefined"
        @mousedown.prevent
        @click="emit('choose', agent)"
      >
        <span class="flex shrink-0" aria-hidden="true">
          <AgentPortrait :who="agent.id" :size="PORTRAIT_SIZES.mention" />
        </span>
        <span class="flex min-w-0 items-baseline gap-2">
          <b class="text-label font-medium text-ink">{{ agent.name }}</b>
          <span class="truncate text-meta text-muted">{{ agent.role }}</span>
        </span>
      </li>
    </ul>
    <p v-else class="px-2 py-1.5 text-caption text-muted">Nenhum agente</p>
  </div>
</template>
