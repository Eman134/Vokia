<script setup lang="ts">
import { nextTick, onMounted, ref, useId } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { trapFocus } from '../backlog/dialogFocus'
import { CANCEL_LABEL } from '../agents/agentFormCopy'

const props = defineProps<{
  title: string
  submitLabel: string
  canSubmit: boolean
  submitting: boolean
  failure: string | null
}>()
const emit = defineEmits<{ submit: []; close: []; dismissFailure: [] }>()

const titleId = useId()
const dialog = ref<HTMLElement>()
const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null

function close(): void {
  if (props.submitting) return
  previousFocus?.focus()
  emit('close')
}

onMounted(async () => {
  await nextTick()
  dialog.value?.querySelector<HTMLElement>('input, select')?.focus()
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
      class="flex max-h-full w-105 max-w-full flex-col gap-4 overflow-y-auto rounded-dialog border border-line bg-surface p-5 shadow-float"
      @keydown.esc.stop="close()"
      @keydown.tab="trapFocus($event, dialog)"
    >
      <h2 :id="titleId" class="text-control font-bold text-ink">{{ title }}</h2>
      <fieldset :disabled="submitting" class="m-0 flex min-w-0 flex-col gap-4 border-0 p-0">
        <slot />
      </fieldset>
      <UiNotice v-if="failure !== null" :text="failure" @close="emit('dismissFailure')" />
      <div class="flex justify-end gap-2">
        <UiButton variant="quiet" size="lg" @press="close()">{{ CANCEL_LABEL }}</UiButton>
        <UiButton variant="primary" size="lg" :disabled="!canSubmit || submitting" @press="emit('submit')">
          {{ submitLabel }}
        </UiButton>
      </div>
    </section>
  </div>
</template>
