<script setup lang="ts">
import VokiaMascot from '../../brand/VokiaMascot.vue'
import StepHeading from './StepHeading.vue'

const MASCOT_SIZE = 84
</script>

<template>
  <div class="flex w-full flex-col gap-7">
    <StepHeading
      title="Boas-vindas à Vokia"
      text="Um time de agentes de IA trabalhando no seu código, no seu computador. São cinco passos rápidos."
    >
      <template #mascot>
        <VokiaMascot look="feliz" :size="MASCOT_SIZE" />
      </template>
    </StepHeading>
  </div>
</template>
