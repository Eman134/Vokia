<script setup lang="ts">
import { computed, nextTick, onMounted, ref, useId, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiField from '../../ui/UiField.vue'
import UiNotice from '../../ui/UiNotice.vue'
import UiSelect from '../../ui/UiSelect.vue'
import { trapFocus } from '../backlog/dialogFocus'
import { AVATAR_PROFILES } from '../identities/avatarProfiles'
import AvatarPicker from '../identities/AvatarPicker.vue'
import type { AgentDraft, AgentProfile } from '../team/teamApi'
import { useRoster } from '../team/useRoster'
import {
  AVATAR_LABEL,
  CANCEL_LABEL,
  CLI_LABEL,
  CREATE_LABEL,
  DIALOG_TITLE,
  EDIT_TITLE,
  MODEL_LABEL,
  NAME_LABEL,
  NO_ROLES_HINT,
  ROLE_LABEL,
  SAVE_LABEL,
} from './agentFormCopy'
import { useRoles } from './useRoles'

const props = defineProps<{ agent?: AgentProfile }>()
const emit = defineEmits<{ saved: [profile: AgentProfile]; close: [] }>()

const roster = useRoster()
const roles = useRoles()
const titleId = useId()
const dialog = ref<HTMLElement>()
const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null

const usedAvatars = new Set(roster.agents.map((agent) => agent.avatar))
const firstPreset = roster.presets[0]

const name = ref(props.agent?.name ?? '')
const role = ref(props.agent?.role ?? '')
const avatar = ref(
  props.agent?.avatar ?? (AVATAR_PROFILES.find((profile) => !usedAvatars.has(profile.id)) ?? AVATAR_PROFILES[0]).id,
)
const cli = ref(props.agent?.cli ?? firstPreset?.id ?? '')
const model = ref(props.agent?.model ?? firstPreset?.defaultModel ?? '')
const submitting = ref(false)
const failure = ref<string | null>(null)

const roleOptions = computed(() => {
  const options = roles.roles.map((candidate) => ({ label: candidate.name, value: candidate.name }))
  // Papel de agente antigo que ainda não está no cadastro continua visível para não ser perdido ao salvar.
  if (role.value !== '' && !options.some((option) => option.value === role.value)) {
    options.push({ label: role.value, value: role.value })
  }
  return options
})
const cliOptions = computed(() => roster.presets.map((preset) => ({ label: preset.label, value: preset.id })))
const preset = computed(() => roster.presets.find((candidate) => candidate.id === cli.value))
const modelOptions = computed(() => (preset.value?.models ?? []).map((candidate) => ({ label: candidate, value: candidate })))
const canSave = computed(
  () => name.value.trim() !== '' && role.value !== '' && cli.value !== '' && model.value !== '' && !submitting.value,
)

watch(cli, () => {
  model.value = preset.value?.defaultModel ?? ''
})

function focusName(): void {
  dialog.value?.querySelector('input')?.focus()
}

async function dismissFailure(): Promise<void> {
  failure.value = null
  await nextTick()
  focusName()
}

function close(): void {
  if (submitting.value) return
  previousFocus?.focus()
  emit('close')
}

async function submit(): Promise<void> {
  if (!canSave.value) return
  submitting.value = true
  failure.value = null
  const draft: AgentDraft = {
    name: name.value,
    role: role.value,
    avatar: avatar.value,
    cli: cli.value,
    model: model.value,
  }
  try {
    const profile = props.agent ? await roster.update(props.agent.id, draft) : await roster.create(draft)
    submitting.value = false
    previousFocus?.focus()
    emit('saved', profile)
    emit('close')
  } catch (error) {
    failure.value = String(error)
    submitting.value = false
    await nextTick()
    focusName()
  }
}

onMounted(focusName)
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
      <h2 :id="titleId" class="text-control font-bold text-ink">{{ agent ? EDIT_TITLE : DIALOG_TITLE }}</h2>
      <fieldset :disabled="submitting" class="m-0 flex min-w-0 flex-col gap-4 border-0 p-0">
        <div class="flex flex-col gap-3" @keydown.enter.prevent="submit()">
          <UiField v-model="name" :label="NAME_LABEL" />
          <label class="flex flex-col gap-1.5">
            <span class="text-label font-medium text-muted">{{ ROLE_LABEL }}</span>
            <UiSelect v-model="role" :options="roleOptions" :label="ROLE_LABEL" placeholder="Escolha um papel" />
          </label>
          <p v-if="roleOptions.length === 0" class="text-caption text-muted">{{ NO_ROLES_HINT }}</p>
        </div>
        <div class="flex flex-col gap-1.5">
          <span :id="`${titleId}-avatar`" class="text-label font-medium text-muted">{{ AVATAR_LABEL }}</span>
          <AvatarPicker v-model="avatar" :label-id="`${titleId}-avatar`" />
        </div>
        <div class="flex gap-3">
          <label class="flex flex-col gap-1.5">
            <span class="text-label font-medium text-muted">{{ CLI_LABEL }}</span>
            <UiSelect v-model="cli" :options="cliOptions" :label="CLI_LABEL" />
          </label>
          <label class="flex flex-col gap-1.5">
            <span class="text-label font-medium text-muted">{{ MODEL_LABEL }}</span>
            <UiSelect v-model="model" :options="modelOptions" :label="MODEL_LABEL" />
          </label>
        </div>
      </fieldset>
      <UiNotice v-if="failure !== null" :text="failure" @close="dismissFailure()" />
      <div class="flex justify-end gap-2">
        <UiButton variant="quiet" size="lg" @press="close()">{{ CANCEL_LABEL }}</UiButton>
        <UiButton variant="primary" size="lg" :disabled="!canSave" @press="submit()">
          {{ agent ? SAVE_LABEL : CREATE_LABEL }}
        </UiButton>
      </div>
    </section>
  </div>
</template>
