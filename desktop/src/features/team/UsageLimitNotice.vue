<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import { useUsageLimit } from '../usage/useUsageLimit'

const RESUME_LABEL = 'Retomar'

const usageLimit = useUsageLimit()
</script>

<template>
  <div
    v-if="usageLimit.visible && usageLimit.resumable"
    class="flex items-center gap-2 text-sm"
    data-testid="usage-limit-notice"
  >
    <UiButton
      variant="quiet"
      size="sm"
      data-testid="usage-limit-resume"
      @press="usageLimit.resume()"
    >
      {{ RESUME_LABEL }}
    </UiButton>
  </div>
</template>
