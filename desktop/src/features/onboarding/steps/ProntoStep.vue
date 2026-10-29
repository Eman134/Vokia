<script setup lang="ts">
import { CURRENT_PROJECT } from '../../../shell/railNav'
import VokiaMascot from '../../brand/VokiaMascot.vue'
import { FINAL_TIPS } from '../onboardingSample'
import StepHeading from './StepHeading.vue'

const MASCOT_SIZE = 92
const TIP_CLASSES = 'flex items-center gap-3 rounded-card border border-line bg-surface px-3.5 py-3'
const TIP_KEY_CLASSES = 'min-w-13 flex-none rounded-control-sm bg-surface-2 px-2 py-1 text-center font-mono text-small'
</script>

<template>
  <div class="flex w-full flex-col gap-7">
    <StepHeading
      title="Tudo pronto"
      :text="`Iara já está lendo o ${CURRENT_PROJECT}. A primeira proposta chega na barra de baixo em uns 2 minutos.`"
    >
      <template #mascot>
        <VokiaMascot look="animado" :size="MASCOT_SIZE" />
      </template>
    </StepHeading>
    <div class="flex flex-col gap-2">
      <div v-for="tip in FINAL_TIPS" :key="tip.key" :class="TIP_CLASSES">
        <span :class="TIP_KEY_CLASSES">{{ tip.key }}</span>
        <span class="text-control">{{ tip.text }}</span>
      </div>
      <span class="text-center text-caption text-muted">
        O que você pulou fica em Configurações, e dá para rever o onboarding de lá.
      </span>
    </div>
  </div>
</template>
