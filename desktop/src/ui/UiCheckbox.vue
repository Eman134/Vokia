<script setup lang="ts">
import UiHandLine from './UiHandLine.vue'
import UiIcon from './UiIcon.vue'
import { DEFAULT_JITTER, sketchCheck } from './sketch/sketchPath'

defineProps<{ checked: boolean }>()
const emit = defineEmits<{ change: [] }>()

const CHECK_BOX = 16
const CHECK_SEED = 7
const CHECK_PATH = sketchCheck(CHECK_BOX, CHECK_BOX, CHECK_SEED, DEFAULT_JITTER)
const BOX_CLASSES =
  'pointer-events-none flex size-full items-center justify-center rounded-sm border bg-surface peer-focus-visible:outline-2 peer-focus-visible:outline-solid peer-focus-visible:outline-ink'
const BOX_CHECKED_CLASSES = 'border-ink-pen text-ink-pen'
const BOX_UNCHECKED_CLASSES = 'border-graphite'
const STRIKE_CLASSES = 'pointer-events-none absolute inset-x-0 top-1/2 h-2 -translate-y-1/2'
</script>

<template>
  <label class="flex cursor-pointer items-start gap-2.5 text-control text-ink">
    <span class="relative mt-0.5 size-4.25 flex-none">
      <!-- A caixa real fica por cima e transparente: teclado, leitor de tela e clique seguem nativos. -->
      <input type="checkbox" :checked="checked" class="peer absolute inset-0 size-full cursor-pointer opacity-0" @change="emit('change')" />
      <span aria-hidden="true" :class="[BOX_CLASSES, checked ? BOX_CHECKED_CLASSES : BOX_UNCHECKED_CLASSES]">
        <UiIcon v-if="checked" :path="CHECK_PATH" :size="CHECK_BOX" />
      </span>
    </span>
    <span :class="['relative min-w-0 flex-1 wrap-break-word', checked && 'text-muted']">
      <slot />
      <UiHandLine v-if="checked" variant="strike" :class="STRIKE_CLASSES" />
    </span>
  </label>
</template>
