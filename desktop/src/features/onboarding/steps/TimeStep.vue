<script setup lang="ts">
import UiButton from '../../../ui/UiButton.vue'
import AgentPortrait from '../../team/AgentPortrait.vue'
import { TEAM_SAMPLES } from '../onboardingSample'
import ChoiceCard from './ChoiceCard.vue'
import StepHeading from './StepHeading.vue'

const PORTRAIT_SIZE = 30

const emit = defineEmits<{ 'skip-team': [] }>()
</script>

<template>
  <div class="flex w-full flex-col gap-7">
    <StepHeading title="Monte o time" text="Comece de um modelo. Dá para trocar agentes, camadas e quem lidera depois, no Mapa do time." />
    <div class="flex flex-col gap-2.5">
      <ChoiceCard v-for="team in TEAM_SAMPLES" :key="team.name" :selected="team.selected">
        <span class="flex min-w-0 flex-col gap-0.5">
          <b class="text-[14.5px] font-semibold">{{ team.name }}</b>
          <span class="text-caption text-muted">{{ team.description }}</span>
        </span>
        <span class="flex">
          <span v-for="(member, index) in team.members" :key="member" :class="index > 0 && '-ml-2'">
            <AgentPortrait :who="member" :size="PORTRAIT_SIZE" />
          </span>
        </span>
      </ChoiceCard>
      <UiButton variant="quiet" size="sm" class="self-center" @press="emit('skip-team')">Começar sem time</UiButton>
    </div>
  </div>
</template>
