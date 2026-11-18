<script setup lang="ts">
import { computed } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import { useUsageLimit } from '../usage/useUsageLimit'
import { usageLimitText } from '../usage/usageLimitCopy'

const RESUME_LABEL = 'Retomar'

const usageLimit = useUsageLimit()
const text = computed(() => (usageLimit.notice === null ? '' : usageLimitText(usageLimit.notice)))
</script>

<template>
  <div v-if="usageLimit.visible" class="flex items-center gap-2 text-sm" data-testid="usage-limit-notice">
    <span class="text-warn-text">{{ text }}</span>
    <UiButton
      v-if="usageLimit.resumable"
      variant="quiet"
      size="sm"
      data-testid="usage-limit-resume"
      @press="usageLimit.resume()"
    >
      {{ RESUME_LABEL }}
    </UiButton>
  </div>
</template>
