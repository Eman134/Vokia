<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import { BACK_LABEL, CONTINUE_TO_REQUEST_LABEL, TEAM_FOOTER_HINT } from './projetosCopy'
import { useNovoProjeto } from './useNovoProjeto'

const wizard = useNovoProjeto()
</script>

<template>
  <div class="flex flex-col gap-5.5">
    <p class="text-body text-muted">{{ TEAM_FOOTER_HINT }}</p>

    <div class="flex items-center justify-end gap-3">
      <UiButton variant="quiet" size="lg" data-testid="step-back" @press="wizard.back">{{ BACK_LABEL }}</UiButton>
      <UiButton variant="primary" size="lg" data-testid="step-continue" @press="wizard.next">
        {{ CONTINUE_TO_REQUEST_LABEL }}
      </UiButton>
    </div>
  </div>
</template>
