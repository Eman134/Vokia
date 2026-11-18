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
  'inline-flex shrink-0 cursor-pointer items-center justify-center rounded-control font-medium focus:outline-2 focus:outline-solid focus:outline-accent disabled:cursor-default disabled:opacity-40'
const SIZE_CLASSES: Record<ButtonSize, string> = {
  sm: 'h-7 px-2.5 text-label',
  lg: 'h-9 px-3.5 text-control',
}
const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary: 'bg-ink text-bg enabled:hover:bg-ink/85',
  approve: 'bg-accent text-accent-ink enabled:hover:bg-accent/85',
  quiet: 'text-muted enabled:hover:bg-hover enabled:hover:text-ink',
  danger: 'bg-danger/10 text-danger enabled:hover:bg-danger/20',
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
