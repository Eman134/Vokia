<script setup lang="ts">
import { onMounted, ref } from 'vue'
import ThemeSwitch from '../theme/ThemeSwitch.vue'
import { useTeam } from '../team/useTeam'
import { useTheme } from '../theme/useTheme'
import { useProjects } from '../projects/useProjects'
import { useNavigation } from '../../shell/useNavigation'
import UiButton from '../../ui/UiButton.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiProgress from '../../ui/UiProgress.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import UiSelect from '../../ui/UiSelect.vue'
import UiSwitch from '../../ui/UiSwitch.vue'
import type { SettingsActionId, SettingsItem } from './configuracoesSample'
import { eraseAllData, fetchSettings, saveSetting } from './settingsApi'

const props = defineProps<{ item: SettingsItem }>()
const projects = useProjects()
const team = useTeam()
const theme = useTheme()
const navigation = useNavigation()
const ONBOARDING_PAGE = 'onboarding'
const ACTION_HANDLERS: Record<SettingsActionId, () => void> = {
  'rever-onboarding': () => navigation.goTo(ONBOARDING_PAGE),
}

// Espelha PERMISSION_GRANT_VALUES de src-tauri/src/db.rs.
const PERMISSION_GRANT_KEY = 'permission_grant'
const GRANT_CHOICES = ['session', 'once'] as const
type GrantChoice = (typeof GRANT_CHOICES)[number]
const DEFAULT_GRANT: GrantChoice = 'session'
const GRANT_LABELS: Record<GrantChoice, string> = { session: 'Por sessão', once: 'Uma vez' }
const GRANT_OPTIONS = GRANT_CHOICES.map((choice) => ({ label: GRANT_LABELS[choice], value: choice }))
const ERROR_CLASSES = 'text-small text-danger'
const STACK_CLASSES = 'flex flex-col items-end gap-1'
// Limita a largura do controle: a mensagem de confirmação é longa e, sem limite, empurra o rótulo para uma coluna estreita.
const ERASE_CLASSES = `${STACK_CLASSES} max-w-md`

const grant = ref<GrantChoice>(DEFAULT_GRANT)
const grantError = ref<string | null>(null)
const erasing = ref(false)
const eraseError = ref<string | null>(null)

async function eraseEverything(): Promise<void> {
  erasing.value = true
  eraseError.value = null
  try {
    await eraseAllData()
  } catch (error) {
    eraseError.value = `Não foi possível apagar os dados: ${String(error)}`
    erasing.value = false
    return
  }
  try {
    await projects.load()
    await team.restoreHistory()
    await theme.load()
    navigation.goTo(ONBOARDING_PAGE)
  } catch (error) {
    eraseError.value = `Dados apagados, mas a tela não recarregou: ${String(error)}`
  } finally {
    erasing.value = false
  }
}

function isGrantChoice(value: string | undefined): value is GrantChoice {
  return GRANT_CHOICES.some((choice) => choice === value)
}

async function loadGrant(): Promise<void> {
  try {
    const stored = (await fetchSettings())[PERMISSION_GRANT_KEY]
    if (isGrantChoice(stored)) {
      grant.value = stored
    }
  } catch (error) {
    grantError.value = `Não foi possível ler a aprovação de comandos: ${String(error)}`
  }
}

async function chooseGrant(next: GrantChoice): Promise<void> {
  grant.value = next
  try {
    await saveSetting(PERMISSION_GRANT_KEY, next)
    grantError.value = null
  } catch (error) {
    grantError.value = `Não foi possível salvar a aprovação de comandos: ${String(error)}`
  }
}

onMounted(() => {
  if (props.item.kind === 'row' && props.item.control.kind === 'permission-grant') {
    void loadGrant()
  }
})

const ROW_CLASSES = 'flex items-center gap-3.5 border-t border-line py-3'
const CONTROL_CLASSES = 'flex flex-none flex-wrap items-center justify-end gap-2'
const INPUT_CLASSES =
  'h-9 w-18 rounded-control border border-line bg-surface px-2.5 text-control text-ink disabled:opacity-40'
const CLI_ROW_CLASSES = 'grid grid-cols-[40px_minmax(0,1fr)_auto] items-center gap-3.5 border-t border-line py-3'
const CLI_CODE_CLASSES = 'grid size-10 place-items-center rounded-control-lg bg-surface-2 font-mono text-label font-medium'
const MOMENTS_CLASSES = 'grid grid-cols-3 gap-2 border-t border-line py-3'
const MOMENT_CLASSES = 'flex items-center gap-2.5 rounded-card border px-3 py-2.5'
const MOMENT_ON_CLASSES = 'border-accent bg-accent-soft'
const MOMENT_OFF_CLASSES = 'border-line bg-surface'
const CODE_CLASSES: Record<'plain' | 'ok' | 'danger', string> = {
  plain: 'font-mono text-caption text-muted',
  ok: 'rounded-chip bg-ok-soft px-2 py-0.5 font-mono text-small text-ok',
  danger: 'rounded-chip bg-danger/10 px-2 py-0.5 font-mono text-small text-danger',
}
const KEY_CLASSES = 'rounded-chip border border-line bg-surface-2 px-1.5 font-mono text-small text-ink'

function toOptions(labels: readonly string[]) {
  return labels.map((label) => ({ label, value: label }))
}
</script>

<template>
  <div v-if="item.kind === 'group'" class="flex flex-col gap-0.5 pt-4 pb-1">
    <span class="text-small font-semibold tracking-[0.04em] text-muted uppercase">{{ item.label }}</span>
    <span v-if="item.hint" class="text-small text-muted">{{ item.hint }}</span>
  </div>

  <p v-else-if="item.kind === 'note'" class="pt-3 text-caption text-muted">{{ item.text }}</p>

  <div v-else-if="item.kind === 'cli'" :class="CLI_ROW_CLASSES">
    <span :class="CLI_CODE_CLASSES" aria-hidden="true">{{ item.code }}</span>
    <div class="flex min-w-0 flex-col gap-1">
      <span class="flex items-baseline gap-2">
        <span class="text-control font-semibold">{{ item.name }}</span>
        <span class="text-caption text-muted">{{ item.version }}</span>
      </span>
      <span class="flex items-center gap-1.5 text-caption text-ink">
        <span class="size-1.75 rounded-full bg-ok" aria-hidden="true" />
        {{ item.status }}
        <span class="text-muted">{{ item.detail }}</span>
      </span>
    </div>
    <UiButton variant="quiet" size="sm" disabled>{{ item.action }}</UiButton>
  </div>

  <div v-else-if="item.kind === 'usage'" class="flex flex-col gap-2 border-t border-line py-3">
    <div class="flex items-baseline gap-2">
      <span class="text-control">{{ item.label }}</span>
      <span class="ml-auto text-caption text-muted">{{ item.detail }}</span>
    </div>
    <UiProgress :value="item.percent" />
  </div>

  <div v-else-if="item.kind === 'moments'" :class="MOMENTS_CLASSES">
    <div
      v-for="moment in item.items"
      :key="moment.label"
      :class="[MOMENT_CLASSES, moment.on ? MOMENT_ON_CLASSES : MOMENT_OFF_CLASSES]"
    >
      <div class="flex min-w-0 flex-1 flex-col gap-0.5">
        <span class="text-control">{{ moment.label }}</span>
        <span class="text-small text-muted">{{ moment.hint }}</span>
      </div>
      <UiSwitch :model-value="moment.on" :label="moment.label" disabled />
    </div>
  </div>

  <div v-else :class="ROW_CLASSES">
    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
      <span class="text-control">{{ item.label }}</span>
      <span v-if="item.hint" class="text-small text-muted">{{ item.hint }}</span>
    </div>

    <div :class="CONTROL_CLASSES">
      <ThemeSwitch v-if="item.control.kind === 'theme'" />
      <div v-else-if="item.control.kind === 'permission-grant'" :class="STACK_CLASSES">
        <UiSegmented
          :options="GRANT_OPTIONS"
          :model-value="grant"
          label="Aprovação de comando"
          @update:model-value="chooseGrant"
        />
        <p v-if="grantError" role="alert" :class="ERROR_CLASSES">{{ grantError }}</p>
      </div>
      <div v-else-if="item.control.kind === 'erase-data'" :class="ERASE_CLASSES">
        <UiConfirm
          label="Apagar…"
          message="Apaga todos os projetos, conversas, backlog, planos e também as configurações (tema e permissões voltam ao padrão). Não pode ser desfeito."
          confirm-label="Apagar tudo"
          :busy="erasing"
          @confirm="eraseEverything"
        />
        <p v-if="eraseError" role="alert" :class="ERROR_CLASSES">{{ eraseError }}</p>
      </div>
      <UiSwitch v-else-if="item.control.kind === 'switch'" :model-value="item.control.on" :label="item.label" disabled />
      <UiSegmented
        v-else-if="item.control.kind === 'segmented'"
        :options="toOptions(item.control.options)"
        :model-value="item.control.selected"
        :label="item.label"
        disabled
      />
      <UiSelect
        v-else-if="item.control.kind === 'select'"
        :options="toOptions(item.control.options)"
        :model-value="item.control.options[0]"
        :label="item.label"
        disabled
      />
      <template v-else-if="item.control.kind === 'input'">
        <input :value="item.control.value" :aria-label="item.label" :class="INPUT_CLASSES" disabled aria-disabled="true" />
        <span v-if="item.control.unit" class="text-caption text-muted">{{ item.control.unit }}</span>
      </template>
      <template v-else-if="item.control.kind === 'value'">
        <span class="text-control font-medium">{{ item.control.text }}</span>
        <span class="text-caption text-muted">{{ item.control.note }}</span>
      </template>
      <template v-else-if="item.control.kind === 'code'">
        <span v-for="value in item.control.values" :key="value" :class="CODE_CLASSES[item.control.tone ?? 'plain']">
          {{ value }}
        </span>
      </template>
      <template v-else-if="item.control.kind === 'keys'">
        <kbd v-for="key in item.control.keys" :key="key" :class="KEY_CLASSES">{{ key }}</kbd>
      </template>
      <template v-else-if="item.control.kind === 'actions'">
        <UiButton
          v-for="action in item.control.actions"
          :key="action.label"
          :variant="action.variant"
          size="sm"
          :disabled="!action.id"
          @press="action.id && ACTION_HANDLERS[action.id]()"
        >
          {{ action.label }}
        </UiButton>
      </template>
    </div>
  </div>
</template>
