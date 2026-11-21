<script setup lang="ts">
const props = defineProps<{ disabled?: boolean }>()
const emit = defineEmits<{ press: [] }>()

const CLASSES =
  'inline-flex h-8 shrink-0 cursor-pointer items-center gap-1.5 rounded-control border border-line bg-surface px-2.5 text-caption font-medium text-ink transition-colors enabled:hover:bg-hover focus:outline-2 focus:outline-solid focus:outline-accent disabled:cursor-not-allowed disabled:opacity-40 motion-reduce:transition-none'

// Botão desabilitado ainda recebe click disparado por código; o guard impede o evento de sair.
function press() {
  if (props.disabled) return
  emit('press')
}
</script>

<template>
  <button type="button" :class="CLASSES" :disabled="disabled" @click="press">
    <svg
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      <line x1="12" y1="5" x2="12" y2="19" />
      <line x1="5" y1="12" x2="19" y2="12" />
    </svg>
    <span><slot /></span>
  </button>
</template>
