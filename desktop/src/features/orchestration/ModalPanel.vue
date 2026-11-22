<script setup lang="ts">
import { nextTick, onMounted, ref, useId } from 'vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import { trapFocus } from '../backlog/dialogFocus'

defineProps<{ title: string }>()
const emit = defineEmits<{ close: [] }>()

const OVERLAY_CLASSES = 'fixed inset-0 z-50 flex items-center justify-center bg-paper/70 p-4'
const FRAME_CLASSES = 'relative border border-line w-120 max-w-full rounded-md bg-surface shadow-paper'
// O backdrop tem p-4 (2rem no total): o diálogo rola por dentro em vez de passar da janela.
const DIALOG_CLASSES = 'flex max-h-[calc(100dvh-2rem)] flex-col gap-4 overflow-y-auto p-5'

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
  <div :class="OVERLAY_CLASSES" @click.self="close()">
    <div :class="FRAME_CLASSES">
      <UiCloseButton class="absolute right-3 top-3 z-10" @press="close()" />
      <section
        ref="dialog"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="titleId"
        tabindex="-1"
        :class="DIALOG_CLASSES"
        @keydown.esc.stop="close()"
        @keydown.tab="trapFocus($event, dialog)"
      >
        <h2 :id="titleId" class="text-control font-bold text-ink">{{ title }}</h2>
        <slot />
      </section>
    </div>
  </div>
</template>
