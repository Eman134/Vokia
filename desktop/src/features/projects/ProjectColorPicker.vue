<script setup lang="ts">
import { PROJECT_COLOR_KEYS, PROJECT_COLORS } from './projectColors'

const model = defineModel<string>({ required: true })

const COLOR_LABEL = 'Cor do projeto'
const SWATCH_BASE_CLASSES =
  'size-8 rounded-full border border-project-line focus:outline-2 focus:outline-solid focus:outline-accent'
const SWATCH_ACTIVE_CLASSES = 'ring-2 ring-accent ring-offset-2 ring-offset-surface'
</script>

<template>
  <div class="flex flex-col gap-1.5">
    <span id="project-color-label" class="text-label font-medium text-muted">{{ COLOR_LABEL }}</span>
    <div role="radiogroup" aria-labelledby="project-color-label" class="flex flex-wrap gap-2.5" data-testid="field-color">
      <button
        v-for="key in PROJECT_COLOR_KEYS"
        :key="key"
        type="button"
        role="radio"
        :aria-checked="model === key"
        :aria-label="PROJECT_COLORS[key].label"
        :title="PROJECT_COLORS[key].label"
        :data-testid="`project-color-${key}`"
        :style="{ backgroundColor: `var(--color-project-${key})` }"
        :class="[SWATCH_BASE_CLASSES, model === key ? SWATCH_ACTIVE_CLASSES : '']"
        @click="model = key"
      />
    </div>
  </div>
</template>
