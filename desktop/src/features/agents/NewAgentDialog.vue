<script setup lang="ts">
import { computed, nextTick, onMounted, ref, useId, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import UiCombobox from '../../ui/UiCombobox.vue'
import UiField from '../../ui/UiField.vue'
import UiNotice from '../../ui/UiNotice.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import UiSelect from '../../ui/UiSelect.vue'
import { trapFocus } from '../backlog/dialogFocus'
import { AVATAR_PROFILES } from '../identities/avatarProfiles'
import AvatarPicker from '../identities/AvatarPicker.vue'
import ProviderModelPickerModal, {
  type ModelSelectionPayload,
} from '../providers/ProviderModelPickerModal.vue'
import { EFFORT_LABELS, normalizeReasoningEffort } from '../providers/providerModelCatalog'
import { useProviders } from '../providers/useProviders'
import type { AgentDraft, AgentProfile, LlmTarget, ModelTier, ReasoningEffort } from '../team/teamApi'
import { useRoster } from '../team/useRoster'
import {
  AVATAR_LABEL,
  CANCEL_LABEL,
  CREATE_LABEL,
  DEFAULT_PROVIDER_LABEL,
  DIALOG_TITLE,
  EDIT_TITLE,
  EFFORT_LEVEL_LABEL,
  EFFORT_LEVEL_OPTIONS,
  FALLBACK_PROVIDER_LABEL,
  MODEL_LABEL,
  NAME_LABEL,
  NO_ROLES_HINT,
  ROLE_LABEL,
  SAVE_LABEL,
} from './agentFormCopy'
import { COMMAND_LABEL, RESTORE_DEFAULT_LABEL } from './agentesCopy'
import { useRoles } from './useRoles'

import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
const props = defineProps<{ agent?: AgentProfile }>()
const emit = defineEmits<{ saved: [profile: AgentProfile]; close: [] }>()

const roster = useRoster()
const roles = useRoles()
const providersStore = useProviders()
const titleId = useId()
const dialog = ref<HTMLElement>()
const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null

const usedAvatars = new Set(roster.agents.map((agent) => agent.avatar))
const firstPreset = roster.presets[0]
// O próprio agente em edição mantém o avatar dele; os demais aparecem como em uso.
const avatarOwners = computed<Record<string, string>>(() =>
  Object.fromEntries(
    roster.agents.filter((agent) => agent.id !== props.agent?.id).map((agent) => [agent.avatar, agent.name]),
  ),
)

const name = ref(props.agent?.name ?? '')
const role = ref(props.agent?.role ?? '')
const avatar = ref(
  props.agent?.avatar ?? (AVATAR_PROFILES.find((profile) => !usedAvatars.has(profile.id)) ?? AVATAR_PROFILES[0]).id,
)
const isBuiltin = computed(() => props.agent?.builtin ?? false)
const followDefaultLlm = ref(props.agent?.followDefaultLlm ?? true)
const cli = ref(props.agent?.cli ?? firstPreset?.id ?? '')
const preset = computed(() => roster.presets.find((candidate) => candidate.id === cli.value))
const modelMode = ref<'tier' | 'custom'>(
  props.agent && props.agent.modelTier == null ? 'custom' : 'tier',
)
const modelTier = ref<ModelTier>(props.agent?.modelTier ?? 'intermediate')
const model = ref(props.agent?.model ?? firstPreset?.defaultModel ?? '')
const reasoningEffort = ref<ReasoningEffort | null>(
  normalizeReasoningEffort(props.agent?.reasoningEffort),
)
const modelPickerOpen = ref(false)
const fallbackSectionRef = ref<HTMLElement>()
const command = ref(props.agent?.command ?? preset.value?.defaultCommand ?? '')
const fallbackCli = ref(props.agent?.fallback[0]?.cli ?? '')
const fallbackTier = ref<ModelTier>(props.agent?.fallback[0]?.modelTier ?? 'intermediate')
const commandId = `${titleId}-command`
const submitting = ref(false)
const failure = ref<string | null>(null)
const discoverError = ref<string | null>(null)
const discoveredModels = ref<string[]>([])

const MODEL_MODE_OPTIONS = [
  { label: 'Por nível', value: 'tier' },
  { label: 'Modelo específico', value: 'custom' },
] as const

const discoveringModels = computed(() => Boolean(providersStore.discovering[cli.value]))

const resolvedTierModel = computed(() => {
  const tierModel = preset.value?.tiers?.[modelTier.value]?.trim()
  if (tierModel) return tierModel
  return preset.value?.defaultModel ?? ''
})

const effectiveModel = computed(() =>
  modelMode.value === 'tier' ? resolvedTierModel.value : model.value.trim(),
)

const commandError = computed(() => {
  if (!failure.value) return undefined
  const text = failure.value.toLowerCase()
  if (text.includes('comando') || text.includes('flag')) {
    return failure.value
  }
  return undefined
})

watch(command, () => {
  if (commandError.value) {
    failure.value = null
  }
})

const roleOptions = computed(() => {
  const options = roles.roles.map((candidate) => ({ label: candidate.name, value: candidate.name }))
  // Papel de agente antigo que ainda não está no cadastro continua visível para não ser perdido ao salvar.
  if (role.value !== '' && !options.some((option) => option.value === role.value)) {
    options.push({ label: role.value, value: role.value })
  }
  return options
})
const cliOptions = computed(() => {
  const candidates = isBuiltin.value
    ? roster.presets.filter((candidate) => (candidate.toolTransport ?? (candidate.supportsMcp ? 'mcp-native' : 'none')) !== 'none')
    : roster.presets
  return candidates.map((candidate) => ({ label: candidate.label, value: candidate.id }))
})
const allowedPickerProviderIds = computed(() =>
  isBuiltin.value ? cliOptions.value.map((option) => option.value) : null,
)
const selectedProviderSupportsPerCommandPermissions = computed(
  () => (preset.value?.toolTransport ?? (preset.value?.supportsMcp ? 'mcp-native' : 'none')) === 'mcp-native',
)
const activeEffortSpec = computed(() => {
  const fromStore = providersStore.providers.find((p) => p.id === cli.value)?.effort
  return fromStore ?? preset.value?.effort ?? null
})
const supportedEffortsForCurrentModel = computed<ReasoningEffort[]>(() => {
  const spec = activeEffortSpec.value
  if (!spec) return []
  const target = effectiveModel.value.toLowerCase()
  for (const rule of spec.modelRules ?? []) {
    const pat = rule.pattern.trim().toLowerCase()
    if (pat && target.includes(pat)) {
      return rule.levels
        .map((lvl) => normalizeReasoningEffort(lvl))
        .filter((lvl): lvl is ReasoningEffort => lvl !== null)
    }
  }
  if (!spec.applyToAllModels) return []
  return spec.levels
    .map((lvl) => normalizeReasoningEffort(lvl))
    .filter((lvl): lvl is ReasoningEffort => lvl !== null)
})
const effortSegmentedOptions = computed(() =>
  supportedEffortsForCurrentModel.value.map((value) => ({
    label: EFFORT_LABELS[value],
    value,
  })),
)
const currentReasoningEffortValue = computed<string>({
  get: () => {
    const current = reasoningEffort.value
    const supported = supportedEffortsForCurrentModel.value
    if (current && supported.includes(current)) return current
    const def = normalizeReasoningEffort(activeEffortSpec.value?.defaultLevel)
    if (def && supported.includes(def)) return def
    return supported[0] ?? ''
  },
  set: (val) => {
    reasoningEffort.value = normalizeReasoningEffort(val)
  },
})
const fallbackCliOptions = computed(() => {
  const compatible = isBuiltin.value
    ? roster.presets.filter((candidate) => (candidate.toolTransport ?? (candidate.supportsMcp ? 'mcp-native' : 'none')) !== 'none')
    : roster.presets
  return [
    { label: 'Usar fallback global', value: '' },
    ...compatible.map((candidate) => ({ label: candidate.label, value: candidate.id })),
  ]
})
const modelOptions = computed(() => {
  const base = preset.value?.models ?? []
  const merged = new Set([...base, ...discoveredModels.value])
  return Array.from(merged)
})

const canSave = computed(() => {
  if (submitting.value) return false
  if (isBuiltin.value) {
    return effectiveModel.value !== ''
  }
  if (name.value.trim() === '' || role.value === '') return false
  if (followDefaultLlm.value) return true
  return cli.value !== '' && effectiveModel.value !== '' && command.value.trim() !== ''
})

let skipCliReset = false

watch(cli, () => {
  discoveredModels.value = []
  discoverError.value = null
  if (skipCliReset) {
    skipCliReset = false
    return
  }
  model.value = preset.value?.defaultModel ?? ''
  command.value = preset.value?.defaultCommand ?? ''
  reasoningEffort.value = null
})

function handlePickerSelect(payload: ModelSelectionPayload): void {
  const changedCli = payload.cli !== cli.value
  if (changedCli) {
    skipCliReset = true
    cli.value = payload.cli
    const nextPreset = roster.presets.find((candidate) => candidate.id === payload.cli)
    command.value = nextPreset?.defaultCommand ?? command.value
  }
  modelMode.value = 'custom'
  model.value = payload.model
  reasoningEffort.value = payload.reasoningEffort
  modelPickerOpen.value = false
}

function handlePickerOpenBackup(): void {
  modelPickerOpen.value = false
  void nextTick(() => {
    fallbackSectionRef.value?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
  })
}

async function refreshModels(): Promise<void> {
  if (!cli.value || discoveringModels.value) return
  discoverError.value = null
  try {
    const result = await providersStore.discover(cli.value)
    discoveredModels.value = result.models
    if (result.warning) {
      discoverError.value = result.warning
    }
  } catch (err) {
    discoverError.value = String(err)
  }
}

function restoreDefaultCommand(): void {
  command.value = preset.value?.defaultCommand ?? ''
  failure.value = null
}

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

function buildFallbackList(): LlmTarget[] {
  if (!fallbackCli.value) return []
  const targetPreset = roster.presets.find((candidate) => candidate.id === fallbackCli.value)
  const targetModel =
    targetPreset?.tiers?.[fallbackTier.value]?.trim() || targetPreset?.defaultModel || ''
  return [
    {
      cli: fallbackCli.value,
      modelTier: fallbackTier.value,
      model: targetModel,
    },
  ]
}

async function submit(): Promise<void> {
  if (!canSave.value) return
  submitting.value = true
  failure.value = null
  const fallback = buildFallbackList()
  try {
    let profile: AgentProfile
    const effectiveEffort =
      supportedEffortsForCurrentModel.value.length > 0 ? reasoningEffort.value : null
    if (isBuiltin.value) {
      profile = await roster.updateEmber({
        cli: cli.value,
        modelTier: modelMode.value === 'tier' ? modelTier.value : null,
        model: effectiveModel.value,
        reasoningEffort: effectiveEffort,
        fallback,
      })
    } else {
      const draft: AgentDraft = {
        name: name.value,
        role: role.value,
        avatar: avatar.value,
        cli: cli.value,
        model: effectiveModel.value,
        modelTier: modelMode.value === 'tier' ? modelTier.value : null,
        reasoningEffort: effectiveEffort,
        command: command.value.trim(),
        followDefaultLlm: followDefaultLlm.value,
        fallback,
      }
      profile = props.agent ? await roster.update(props.agent.id, draft) : await roster.create(draft)
    }
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
      class="relative flex max-h-full w-105 max-w-full flex-col gap-4 overflow-y-auto rounded-md border border-line bg-surface p-5 shadow-paper"
      @keydown.esc.stop="close()"
      @keydown.tab="trapFocus($event, dialog)"
    >
      <UiCloseButton class="absolute right-3 top-3 z-10" @press="close()" />
      <h2 :id="titleId" class="text-control font-bold text-ink">{{ agent ? EDIT_TITLE : DIALOG_TITLE }}</h2>
      <fieldset :disabled="submitting" class="m-0 flex min-w-0 flex-col gap-4 border-0 p-0">
        <div class="flex flex-col gap-3" @keydown.enter.prevent="submit()">
          <UiField v-model="name" :label="NAME_LABEL" :disabled="isBuiltin" />
          <label class="flex flex-col gap-1.5">
            <span class="text-label font-medium text-muted">{{ ROLE_LABEL }}</span>
            <UiSelect
              v-model="role"
              :options="roleOptions"
              :label="ROLE_LABEL"
              :disabled="isBuiltin"
              placeholder="Escolha um papel"
            />
          </label>
          <p v-if="roleOptions.length === 0" class="text-caption text-muted">{{ NO_ROLES_HINT }}</p>
        </div>
        <div v-if="!isBuiltin" class="flex flex-col gap-1.5">
          <span :id="`${titleId}-avatar`" class="text-label font-medium text-muted">{{ AVATAR_LABEL }}</span>
          <AvatarPicker v-model="avatar" :label-id="`${titleId}-avatar`" :unavailable="avatarOwners" />
        </div>

        <label v-if="!isBuiltin" class="flex cursor-pointer items-center gap-2 text-caption text-ink">
          <input v-model="followDefaultLlm" type="checkbox" class="size-4 accent-accent" />
          <span>Seguir padrão global do Vokia</span>
        </label>

        <template v-if="isBuiltin || !followDefaultLlm">
          <div class="flex flex-col gap-1.5">
            <div class="flex items-center justify-between gap-2">
              <span class="text-label font-medium text-muted">{{ DEFAULT_PROVIDER_LABEL }}</span>
              <button
                type="button"
                data-testid="agent-open-model-picker"
                :class="`cursor-pointer rounded-sm px-1.5 py-0.5 text-small font-medium text-accent hover:bg-hover ${INK_FOCUS_CLASSES}`"
                @click="modelPickerOpen = true"
              >
                Abrir catálogo
              </button>
            </div>
            <UiSelect
              v-model="cli"
              data-testid="agent-default-provider"
              :options="cliOptions"
              :label="DEFAULT_PROVIDER_LABEL"
            />
            <p
              v-if="isBuiltin && !selectedProviderSupportsPerCommandPermissions"
              class="text-small text-muted"
            >
              Este provedor segue o nível de sandbox da CLI sem confirmação comando a comando.
            </p>
          </div>

          <div class="flex flex-col gap-2">
            <div class="flex items-center justify-between gap-2">
              <span class="text-label font-medium text-muted">{{ EFFORT_LEVEL_LABEL }}</span>
              <UiSegmented
                v-model="modelMode"
                :options="MODEL_MODE_OPTIONS"
                :label="EFFORT_LEVEL_LABEL"
              />
            </div>

            <template v-if="modelMode === 'tier'">
              <UiSelect
                v-model="modelTier"
                data-testid="agent-tier-select"
                :options="EFFORT_LEVEL_OPTIONS"
                :label="EFFORT_LEVEL_LABEL"
              />
              <p v-if="resolvedTierModel" class="text-small text-muted">
                Resolve para: <span class="font-mono text-ink">{{ resolvedTierModel }}</span>
              </p>
            </template>

            <template v-else>
              <label class="flex flex-col gap-1.5">
                <span class="text-label font-medium text-muted">{{ MODEL_LABEL }}</span>
                <UiCombobox
                  v-model="model"
                  data-testid="agent-model-combobox"
                  :options="modelOptions"
                  :loading="discoveringModels"
                  :show-refresh="true"
                  refresh-label="Atualizar modelos"
                  placeholder="Digite ou selecione um modelo"
                  @refresh="refreshModels()"
                />
              </label>
              <p v-if="discoverError" class="text-small text-warn-text">{{ discoverError }}</p>
            </template>

            <div
              v-if="effortSegmentedOptions.length > 0"
              data-testid="agent-reasoning-effort"
              class="flex items-center justify-between gap-2 pt-1"
            >
              <span class="text-label font-medium text-muted">Esforço de raciocínio</span>
              <UiSegmented
                v-model="currentReasoningEffortValue"
                :options="effortSegmentedOptions"
                label="Esforço de raciocínio"
              />
            </div>
          </div>
        </template>

        <div ref="fallbackSectionRef" class="flex gap-3">
          <label class="flex min-w-0 flex-1 flex-col gap-1.5">
            <span class="text-label font-medium text-muted">{{ FALLBACK_PROVIDER_LABEL }}</span>
            <UiSelect
              v-model="fallbackCli"
              data-testid="agent-fallback-provider"
              :options="fallbackCliOptions"
              :label="FALLBACK_PROVIDER_LABEL"
            />
          </label>
          <label v-if="fallbackCli !== ''" class="flex min-w-0 flex-1 flex-col gap-1.5">
            <span class="text-label font-medium text-muted">{{ EFFORT_LEVEL_LABEL }}</span>
            <UiSelect
              v-model="fallbackTier"
              :options="EFFORT_LEVEL_OPTIONS"
              :label="EFFORT_LEVEL_LABEL"
            />
          </label>
        </div>

        <div v-if="!isBuiltin && !followDefaultLlm" class="flex flex-col gap-1.5">
          <div class="flex items-baseline justify-between gap-3">
            <label :for="commandId" class="text-label font-medium text-muted">{{ COMMAND_LABEL }}</label>
            <button
              type="button"
              class="cursor-pointer text-caption text-muted hover:text-ink hover:underline disabled:cursor-not-allowed disabled:opacity-50"
              :disabled="submitting || command === (preset?.defaultCommand ?? '')"
              @click="restoreDefaultCommand()"
            >
              {{ RESTORE_DEFAULT_LABEL }}
            </button>
          </div>
          <input
            :id="commandId"
            v-model="command"
            :disabled="submitting"
            :class="[
              `h-9 w-full min-w-0 rounded-md border bg-surface px-3 font-mono text-control text-ink placeholder:text-faint ${INK_FOCUS_CLASSES}`,
              commandError ? 'border-danger' : 'border-line',
            ]"
            :aria-invalid="Boolean(commandError)"
            @keydown.enter.prevent="submit()"
          />
          <span v-if="commandError" role="alert" class="text-small text-danger">{{ commandError }}</span>
        </div>
      </fieldset>
      <UiNotice v-if="failure !== null && !commandError" :text="failure" @close="dismissFailure()" />
      <div class="flex justify-end gap-2">
        <UiButton variant="quiet" size="lg" @press="close()">{{ CANCEL_LABEL }}</UiButton>
        <UiButton variant="primary" size="lg" :disabled="!canSave" @press="submit()">
          {{ agent ? SAVE_LABEL : CREATE_LABEL }}
        </UiButton>
      </div>
    </section>

    <ProviderModelPickerModal
      :open="modelPickerOpen"
      title="Selecionar modelo do agente"
      :selected-provider-id="cli"
      :selected-model="model"
      :selected-model-tier="modelMode === 'tier' ? modelTier : null"
      :selected-effort="reasoningEffort"
      :allowed-provider-ids="allowedPickerProviderIds"
      :show-backup-shortcut="true"
      :show-overrides="false"
      @close="modelPickerOpen = false"
      @select="handlePickerSelect"
      @open-backup="handlePickerOpenBackup()"
    />
  </div>
</template>
