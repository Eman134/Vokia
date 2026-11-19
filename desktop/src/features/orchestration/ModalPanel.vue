<script setup lang="ts">
import { nextTick, onMounted, ref, useId } from 'vue'
import { trapFocus } from '../backlog/dialogFocus'

defineProps<{ title: string }>()
const emit = defineEmits<{ close: [] }>()

const titleId = useId()
const dialog = ref<HTMLElement>()
const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null

function focusFirst(): void {
  dialog.value?.querySelector<HTMLElement>('input, textarea, button')?.focus()
}

function close(): void {
  previousFocus?.focus()
  emit('close')
}

onMounted(async () => {
  await nextTick()
  focusFirst()
})
</script>

<template>
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4" @click.self="close()">
    <section
      ref="dialog"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      tabindex="-1"
      class="flex max-h-full w-120 max-w-full flex-col gap-4 overflow-y-auto rounded-dialog border border-line bg-surface p-5 shadow-float"
      @keydown.esc.stop="close()"
      @keydown.tab="trapFocus($event, dialog)"
    >
      <h2 :id="titleId" class="text-control font-bold text-ink">{{ title }}</h2>
      <slot />
    </section>
  </div>
</template>
