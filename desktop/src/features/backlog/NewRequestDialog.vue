<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import { useProjects } from '../projects/useProjects'
import {
  NEW_REQUEST_CANCEL_LABEL,
  NEW_REQUEST_CREATING_LABEL,
  NEW_REQUEST_DESCRIPTION_LABEL,
  NEW_REQUEST_DESCRIPTION_PLACEHOLDER,
  NEW_REQUEST_DIALOG_TITLE,
  NEW_REQUEST_SUBMIT_LABEL,
  NEW_REQUEST_TITLE_LABEL,
  NEW_REQUEST_TITLE_PLACEHOLDER,
} from './tasksPageCopy'
import { trapFocus } from './dialogFocus'
import { useBacklog } from './useBacklog'

import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
const OVERLAY_CLASSES = 'fixed inset-0 z-50 flex items-center justify-center bg-paper/70 p-4'
const FRAME_CLASSES = 'relative border border-line w-105 max-w-full rounded-md bg-surface shadow-paper'
// O backdrop tem p-4 (2rem no total): o diálogo rola por dentro em vez de passar da janela.
const DIALOG_CLASSES = 'flex max-h-[calc(100dvh-2rem)] flex-col gap-3 overflow-y-auto p-5'
const TITLE_FIELD_SELECTOR = 'input'
const DESCRIPTION_ROWS = 4
const LABEL_CLASSES = 'text-label font-medium text-muted'
const CONTROL_CLASSES =
  `w-full min-w-0 rounded-md border border-line bg-surface px-3 text-control text-ink placeholder:text-faint ${INK_FOCUS_CLASSES}`

const backlog = useBacklog()
const projects = useProjects()
const titleId = useId()
const fieldTitleId = useId()
const descriptionId = useId()
const dialog = ref<HTMLElement>()
let previousFocus: HTMLElement | null = null

const draft = computed(() => backlog.requestDraft)
const isOpen = computed(() => draft.value !== null)
const canSubmit = computed(() => !backlog.creating && (draft.value?.title.trim() ?? '') !== '')

watch(isOpen, async (open) => {
  if (!open) {
    previousFocus?.focus()
    previousFocus = null
    return
  }
  previousFocus ??= document.activeElement instanceof HTMLElement ? document.activeElement : null
  await nextTick()
  dialog.value?.querySelector<HTMLElement>(TITLE_FIELD_SELECTOR)?.focus()
})

watch(
  () => projects.selected?.id,
  () => backlog.discardRequest(),
)

function submit(): void {
  if (canSubmit.value) void backlog.submitRequest()
}
</script>

<template>
  <div v-if="draft" :class="OVERLAY_CLASSES" @click.self="backlog.cancelRequest()">
    <div :class="FRAME_CLASSES">
      <UiCloseButton class="absolute right-3 top-3 z-10" @press="backlog.cancelRequest()" />
    <section
      ref="dialog"
      data-testid="new-request-dialog"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      tabindex="-1"
      :class="DIALOG_CLASSES"
      @keydown.esc.stop="backlog.cancelRequest()"
      @keydown.tab="trapFocus($event, dialog)"
    >
      <b :id="titleId" class="text-control text-ink">{{ NEW_REQUEST_DIALOG_TITLE }}</b>
      <label class="flex flex-col gap-1.5" :for="fieldTitleId">
        <span :class="LABEL_CLASSES">{{ NEW_REQUEST_TITLE_LABEL }}</span>
        <input
          :id="fieldTitleId"
          v-model="draft.title"
          data-testid="new-request-title"
          :placeholder="NEW_REQUEST_TITLE_PLACEHOLDER"
          :class="[CONTROL_CLASSES, 'h-9']"
          @keydown.enter="submit"
        />
      </label>
      <label class="flex flex-col gap-1.5" :for="descriptionId">
        <span :class="LABEL_CLASSES">{{ NEW_REQUEST_DESCRIPTION_LABEL }}</span>
        <textarea
          :id="descriptionId"
          v-model="draft.description"
          data-testid="new-request-description"
          :placeholder="NEW_REQUEST_DESCRIPTION_PLACEHOLDER"
          :rows="DESCRIPTION_ROWS"
          class="py-2"
          :class="CONTROL_CLASSES"
        />
      </label>
      <div class="flex justify-end gap-2">
        <UiButton
          data-testid="new-request-cancel"
          variant="quiet"
          size="lg"
          :disabled="backlog.creating"
          @press="backlog.cancelRequest()"
        >
          {{ NEW_REQUEST_CANCEL_LABEL }}
        </UiButton>
        <UiButton data-testid="new-request-submit" variant="primary" size="lg" :disabled="!canSubmit" @press="submit">
          {{ backlog.creating ? NEW_REQUEST_CREATING_LABEL : NEW_REQUEST_SUBMIT_LABEL }}
        </UiButton>
      </div>
    </section>
    </div>
  </div>
</template>
