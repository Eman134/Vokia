<script setup lang="ts">
import { INK_FOCUS_CLASSES } from './handDrawn'
import UiHandLine from './UiHandLine.vue'
type ButtonVariant = 'primary' | 'approve' | 'quiet' | 'danger'
type ButtonSize = 'sm' | 'lg'

const props = defineProps<{
  variant: ButtonVariant
  size: ButtonSize
  disabled?: boolean
}>()
const emit = defineEmits<{ press: [] }>()

const BASE_CLASSES =
  `group relative inline-flex shrink-0 cursor-pointer items-center justify-center rounded-md font-medium ${INK_FOCUS_CLASSES} disabled:cursor-not-allowed disabled:opacity-40`
const SIZE_CLASSES: Record<ButtonSize, string> = {
  sm: 'h-7 px-2.5 text-label',
  lg: 'h-9 px-3.5 text-control',
}
// Primário afunda no clique: a sombra de papel some e o botão desce 1px para onde ela estava.
const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary:
    'bg-ink text-bg shadow-paper enabled:hover:-translate-x-px enabled:hover:-translate-y-px enabled:hover:bg-ink/70 enabled:active:translate-x-px enabled:active:translate-y-px enabled:active:shadow-none',
  approve: 'bg-ink-pen text-accent-ink enabled:hover:bg-ink-pen/85',
  quiet: 'text-muted enabled:hover:text-ink',
  danger: 'bg-danger/10 text-danger enabled:hover:bg-danger/20',
}
const QUIET_UNDERLINE_CLASSES =
  'pointer-events-none absolute inset-x-2 bottom-0.5 h-1.5 opacity-0 group-enabled:group-hover:opacity-100'

// Botão desabilitado ainda recebe click disparado por código; o guard impede o evento de sair.
function press() {
  if (props.disabled) return
  emit('press')
}
</script>

<template>
  <button
    type="button"
    :class="[BASE_CLASSES, SIZE_CLASSES[size], VARIANT_CLASSES[variant]]"
    :disabled="disabled"
    @click="press"
  >
    <slot />
    <UiHandLine v-if="variant === 'quiet'" :class="QUIET_UNDERLINE_CLASSES" />
  </button>
</template>
