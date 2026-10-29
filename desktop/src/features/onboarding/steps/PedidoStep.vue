<script setup lang="ts">
import UiChip from '../../../ui/UiChip.vue'
import { REQUEST_SAMPLES } from '../onboardingSample'
import StepHeading from './StepHeading.vue'

const TEXTAREA_CLASSES =
  'w-full resize-none rounded-panel border-[1.5px] border-accent bg-surface px-[18px] py-4 text-[15px] leading-[1.55] text-ink outline-none'

const prefilled = REQUEST_SAMPLES.find((request) => request.selected)
</script>

<template>
  <div class="flex w-full flex-col gap-7">
    <StepHeading
      title="O que o time faz primeiro?"
      text="Iara transforma o pedido em uma feature com cards e espera o seu ok antes de começar."
    />
    <div class="flex flex-col gap-3">
      <textarea
        aria-label="Primeiro pedido"
        rows="4"
        disabled
        :value="prefilled?.text"
        :class="TEXTAREA_CLASSES"
      />
      <div class="flex flex-wrap justify-center gap-2">
        <UiChip
          v-for="request in REQUEST_SAMPLES"
          :key="request.label"
          :label="request.label"
          :selected="request.selected"
          disabled
        />
      </div>
    </div>
  </div>
</template>
