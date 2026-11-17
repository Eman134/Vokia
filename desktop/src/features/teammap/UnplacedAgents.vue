<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import type { AgentProfile } from '../team/teamApi'
import { PLACE_LABEL, UNPLACED_TITLE } from './teamMapLayers'

const PORTRAIT_SIZE = 28

defineProps<{ agents: AgentProfile[] }>()
const emit = defineEmits<{ place: [agent: AgentProfile] }>()
</script>

<template>
  <section class="flex flex-col gap-3 rounded-[24px] border border-line bg-surface p-4.5">
    <h2 class="text-control font-bold text-ink">{{ UNPLACED_TITLE }}</h2>
    <ul class="m-0 flex list-none flex-wrap gap-2.5 p-0">
      <li v-for="agent in agents" :key="agent.id" class="flex items-center gap-2.5 rounded-[16px] border border-dashed border-line px-2.5 py-2">
        <AgentPortrait :who="agent.id" :size="PORTRAIT_SIZE" />
        <span class="text-control font-semibold text-ink">{{ agent.name }}</span>
        <UiButton variant="quiet" size="sm" @press="emit('place', agent)">{{ PLACE_LABEL }}</UiButton>
      </li>
    </ul>
  </section>
</template>
