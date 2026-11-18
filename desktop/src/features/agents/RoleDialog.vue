<script setup lang="ts">
import { nextTick, onMounted, ref, useId } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiField from '../../ui/UiField.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { trapFocus } from '../backlog/dialogFocus'
import { CANCEL_LABEL, CREATE_LABEL, SAVE_LABEL } from './agentFormCopy'
import {
  ROLE_DESCRIPTION_LABEL,
  ROLE_DIALOG_EDIT_TITLE,
  ROLE_DIALOG_NEW_TITLE,
  ROLE_INSTRUCTIONS_LABEL,
  ROLE_NAME_LABEL,
} from './agentesCopy'
import type { Role } from './rolesApi'
import { useRoles } from './useRoles'

const props = defineProps<{ role?: Role }>()
const emit = defineEmits<{ saved: [role: Role]; close: [] }>()

const roles = useRoles()
const titleId = useId()
const dialog = ref<HTMLElement>()
const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null

const name = ref(props.role?.name ?? '')
const description = ref(props.role?.description ?? '')
const instructions = ref(props.role?.instructions ?? '')
const submitting = ref(false)
const failure = ref<string | null>(null)

const TEXTAREA_CLASSES =
  'min-h-40 w-full min-w-0 rounded-control border border-line bg-surface px-3 py-2 text-control text-ink placeholder:text-faint focus:outline-2 focus:outline-solid focus:outline-accent'
const READ_ONLY_NAME_CLASSES = 'flex h-9 items-center rounded-control border border-line bg-surface px-3 text-control text-muted'

function focusFirst(): void {
  dialog.value?.querySelector<HTMLElement>('input, textarea')?.focus()
}

async function dismissFailure(): Promise<void> {
  failure.value = null
  await nextTick()
  focusFirst()
}

function close(): void {
  if (submitting.value) return
  previousFocus?.focus()
  emit('close')
}

async function submit(): Promise<void> {
  if (submitting.value) return
  submitting.value = true
  failure.value = null
  try {
    const saved = props.role
      ? await roles.update(props.role.id, { description: description.value, instructions: instructions.value })
      : await roles.create({ name: name.value, description: description.value, instructions: instructions.value })
    submitting.value = false
    previousFocus?.focus()
    emit('saved', saved)
    emit('close')
  } catch (error) {
    failure.value = String(error)
    submitting.value = false
    await nextTick()
    focusFirst()
  }
}

onMounted(focusFirst)
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
      <h2 :id="titleId" class="text-control font-bold text-ink">
        {{ role ? ROLE_DIALOG_EDIT_TITLE : ROLE_DIALOG_NEW_TITLE }}
      </h2>
      <fieldset :disabled="submitting" class="m-0 flex min-w-0 flex-col gap-4 border-0 p-0">
        <div v-if="role" class="flex flex-col gap-1.5">
          <span class="text-label font-medium text-muted">{{ ROLE_NAME_LABEL }}</span>
          <span :class="READ_ONLY_NAME_CLASSES">{{ role.name }}</span>
        </div>
        <UiField v-else v-model="name" :label="ROLE_NAME_LABEL" />
        <UiField v-model="description" :label="ROLE_DESCRIPTION_LABEL" />
        <label class="flex flex-col gap-1.5">
          <span class="text-label font-medium text-muted">{{ ROLE_INSTRUCTIONS_LABEL }}</span>
          <textarea v-model="instructions" :class="TEXTAREA_CLASSES" rows="8" />
        </label>
      </fieldset>
      <UiNotice v-if="failure !== null" :text="failure" @close="dismissFailure()" />
      <div class="flex justify-end gap-2">
        <UiButton variant="quiet" size="lg" @press="close()">{{ CANCEL_LABEL }}</UiButton>
        <UiButton variant="primary" size="lg" :disabled="submitting || (!role && name.trim() === '')" @press="submit()">
          {{ role ? SAVE_LABEL : CREATE_LABEL }}
        </UiButton>
      </div>
    </section>
  </div>
</template>
