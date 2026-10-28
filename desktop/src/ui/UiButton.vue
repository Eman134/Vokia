<script setup lang="ts">
type ButtonVariant = 'primary' | 'approve' | 'quiet' | 'danger'
type ButtonSize = 'sm' | 'lg'

const props = defineProps<{
  variant: ButtonVariant
  size: ButtonSize
  disabled?: boolean
}>()
const emit = defineEmits<{ press: [] }>()

const BASE_CLASSES =
  'inline-flex shrink-0 items-center justify-center rounded-control font-medium focus:outline-2 focus:outline-solid focus:outline-accent disabled:opacity-40'
const SIZE_CLASSES: Record<ButtonSize, string> = {
  sm: 'h-7 px-2.5 text-label',
  lg: 'h-9 px-3.5 text-control',
}
const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary: 'bg-ink text-bg',
  approve: 'bg-accent text-accent-ink',
  quiet: 'text-muted',
  danger: 'bg-danger/10 text-danger',
}

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
  </button>
</template>
