<script setup lang="ts">
import { computed } from 'vue'
import UiButton from '../../ui/UiButton.vue'

const props = withDefaults(defineProps<{ label: string; min: number; max: number; step?: number; disabled?: boolean }>(), {
  step: 1,
  disabled: false,
})
const model = defineModel<number>({ required: true })

const DECREASE_LABEL = '−'
const INCREASE_LABEL = '+'
const VALUE_CLASSES = 'min-w-10 text-center font-mono text-control text-ink tabular-nums'

const canDecrease = computed(() => !props.disabled && model.value - props.step >= props.min)
const canIncrease = computed(() => !props.disabled && model.value + props.step <= props.max)

function adjust(delta: number): void {
  const next = Math.min(props.max, Math.max(props.min, model.value + delta))
  if (next !== model.value) model.value = next
}
</script>

<template>
  <div class="inline-flex items-center gap-2" role="group" :aria-label="label">
    <UiButton variant="quiet" size="sm" :disabled="!canDecrease" :aria-label="`Diminuir ${label}`" @press="adjust(-step)">
      {{ DECREASE_LABEL }}
    </UiButton>
    <span :class="VALUE_CLASSES" :aria-label="label">{{ model }}</span>
    <UiButton variant="quiet" size="sm" :disabled="!canIncrease" :aria-label="`Aumentar ${label}`" @press="adjust(step)">
      {{ INCREASE_LABEL }}
    </UiButton>
  </div>
</template>
