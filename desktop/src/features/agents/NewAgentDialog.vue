<script setup lang="ts">
import { computed, nextTick, onMounted, ref, useId, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiField from '../../ui/UiField.vue'
import UiNotice from '../../ui/UiNotice.vue'
import UiSelect from '../../ui/UiSelect.vue'
import { trapFocus } from '../backlog/dialogFocus'
import { AVATAR_PROFILES } from '../identities/avatarProfiles'
import AvatarPicker from '../identities/AvatarPicker.vue'
import type { AgentProfile } from '../team/teamApi'
import { useRoster } from '../team/useRoster'
import {
  AVATAR_LABEL,
  CANCEL_LABEL,
  CLI_LABEL,
  CREATE_LABEL,
  DIALOG_TITLE,
  MODEL_LABEL,
  NAME_LABEL,
  ROLE_LABEL,
} from './agentFormCopy'


const emit = defineEmits<{ created: [agent: AgentProfile]; close: [] }>()

const roster = useRoster()
const titleId = useId()
const dialog = ref<HTMLElement>()
const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null

const usedAvatars = new Set(roster.agents.map((agent) => agent.avatar))
const firstPreset = roster.presets[0]

const name = ref('')
const role = ref('')
const avatar = ref((AVATAR_PROFILES.find((profile) => !usedAvatars.has(profile.id)) ?? AVATAR_PROFILES[0]).id)
const cli = ref(firstPreset?.id ?? '')
const model = ref(firstPreset?.defaultModel ?? '')
const submitting = ref(false)
const failure = ref<string | null>(null)

const cliOptions = computed(() => roster.presets.map((preset) => ({ label: preset.label, value: preset.id })))
const preset = computed(() => roster.presets.find((candidate) => candidate.id === cli.value))
const modelOptions = computed(() => (preset.value?.models ?? []).map((candidate) => ({ label: candidate, value: candidate })))
const canCreate = computed(
  () => name.value.trim() !== '' && role.value.trim() !== '' && cli.value !== '' && model.value !== '' && !submitting.value,
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
  if (!canCreate.value) return
  submitting.value = true
  failure.value = null
  try {
    const profile = await roster.create({
      name: name.value,
      role: role.value,
      avatar: avatar.value,
      cli: cli.value,
      model: model.value,
    })
    submitting.value = false
    previousFocus?.focus()
    emit('created', profile)
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
      <h2 :id="titleId" class="text-control font-bold text-ink">{{ DIALOG_TITLE }}</h2>
      <fieldset :disabled="submitting" class="m-0 flex min-w-0 flex-col gap-4 border-0 p-0">
      <div class="flex flex-col gap-3" @keydown.enter.prevent="submit()">
        <UiField v-model="name" :label="NAME_LABEL" />
        <UiField v-model="role" :label="ROLE_LABEL" />
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
        <UiButton variant="primary" size="lg" :disabled="!canCreate" @press="submit()">{{ CREATE_LABEL }}</UiButton>
      </div>
    </section>
  </div>
</template>
