<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiNotice from '../../ui/UiNotice.vue'
import {
  DELETE_CONFIRM_LABEL,
  DELETE_LABEL,
  EDIT_LABEL,
  NEW_ROLE_LABEL,
  NO_ROLES_TEXT,
  ROLE_DELETE_MESSAGE,
  ROLE_EMPTY_DESCRIPTION,
  ROLE_EMPTY_INSTRUCTIONS,
} from './agentesCopy'
import RoleDialog from './RoleDialog.vue'
import type { Role } from './rolesApi'
import { useRoles } from './useRoles'

const LIST_CLASSES = 'flex w-full min-w-0 flex-col gap-3 rounded-[28px] bg-surface p-5'
const FICHA_CLASSES = 'flex w-full min-w-0 flex-col gap-4 rounded-[28px] border border-line p-6'
const ROW_BASE_CLASSES = 'flex cursor-pointer flex-col gap-0.5 rounded-control border px-3 py-2 text-left'
const ROW_ACTIVE_CLASSES = 'border-accent bg-accent-soft'
const ROW_INACTIVE_CLASSES = 'border-line hover:bg-hover'
const COLUMNS_CLASSES = 'grid min-w-0 grid-cols-1 items-start gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(0,380px)] xl:grid-cols-[minmax(0,1fr)_minmax(0,420px)]'
const BODY_CLASSES = 'whitespace-pre-wrap text-body text-ink'

const roles = useRoles()
const selectedId = ref<string | null>(null)
const dialog = ref<{ role?: Role } | null>(null)
const failure = ref<string | null>(null)
const removing = ref(false)

const selected = computed(
  () => roles.roles.find((role) => role.id === selectedId.value) ?? roles.roles[0] ?? null,
)

function onSaved(role: Role): void {
  selectedId.value = role.id
}

async function removeSelected(): Promise<void> {
  const role = selected.value
  if (!role || removing.value) return
  removing.value = true
  failure.value = null
  try {
    await roles.remove(role.id)
    selectedId.value = null
  } catch (error) {
    failure.value = String(error)
  } finally {
    removing.value = false
  }
}
</script>

<template>
  <div :class="COLUMNS_CLASSES">
    <div :class="LIST_CLASSES">
      <UiButton variant="primary" size="lg" @press="dialog = {}">{{ NEW_ROLE_LABEL }}</UiButton>
      <p v-if="roles.roles.length === 0" class="text-caption text-muted">{{ NO_ROLES_TEXT }}</p>
      <button
        v-for="role in roles.roles"
        :key="role.id"
        type="button"
        :class="[ROW_BASE_CLASSES, selected?.id === role.id ? ROW_ACTIVE_CLASSES : ROW_INACTIVE_CLASSES]"
        :aria-pressed="selected?.id === role.id"
        @click="selectedId = role.id"
      >
        <b class="truncate text-ink">{{ role.name }}</b>
        <span class="truncate text-caption text-muted">{{ role.description || ROLE_EMPTY_DESCRIPTION }}</span>
      </button>
    </div>

    <aside v-if="selected" :class="FICHA_CLASSES">
      <div class="flex flex-col gap-1">
        <b class="text-control text-ink">{{ selected.name }}</b>
        <span class="text-caption text-muted">{{ selected.description || ROLE_EMPTY_DESCRIPTION }}</span>
      </div>
      <p :class="BODY_CLASSES">{{ selected.instructions || ROLE_EMPTY_INSTRUCTIONS }}</p>
      <UiNotice v-if="failure !== null" :text="failure" @close="failure = null" />
      <div class="flex flex-wrap items-center justify-end gap-2">
        <UiConfirm
          :label="DELETE_LABEL"
          :message="ROLE_DELETE_MESSAGE"
          :confirm-label="DELETE_CONFIRM_LABEL"
          :busy="removing"
          @confirm="removeSelected()"
        />
        <UiButton variant="quiet" size="lg" @press="dialog = { role: selected }">{{ EDIT_LABEL }}</UiButton>
      </div>
    </aside>

    <RoleDialog v-if="dialog" :role="dialog.role" @saved="onSaved" @close="dialog = null" />
  </div>
</template>
