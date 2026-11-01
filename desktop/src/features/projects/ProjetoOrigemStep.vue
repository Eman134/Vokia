<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import UiField from '../../ui/UiField.vue'
import { storeToRefs } from 'pinia'
import {
  CANCEL_LABEL,
  CHOOSE_LABEL,
  LINKED_REPO_LABEL,
  PICK_FOLDER_LABEL,
  START_FROM_LABEL,
  START_FROM_VALUE,
} from './projetosSample'
import ProjectIcon from './ProjectIcon.vue'
import { PROJECT_ICON_KEYS, PROJECT_ICONS } from './projectIcons'
import type { CloneMode, OriginKind } from './projectsApi'
import { useNovoProjeto } from './useNovoProjeto'

interface ToggleOption<T extends string> {
  label: string
  value: T
  testid: string
}

const ORIGIN_OPTIONS: readonly ToggleOption<OriginKind>[] = [
  { label: 'Pasta', value: 'folder', testid: 'origin-folder' },
  { label: 'GitHub', value: 'github', testid: 'origin-github' },
]
const MODE_OPTIONS: readonly ToggleOption<CloneMode>[] = [
  { label: 'Link', value: 'link', testid: 'mode-link' },
  { label: 'Fork', value: 'fork', testid: 'mode-fork' },
]

const GROUP_CLASSES = 'flex w-fit gap-0.5 rounded-control bg-surface-2 p-0.75'
const OPTION_CLASSES = 'whitespace-nowrap rounded-control-sm px-3 py-1.25 text-caption focus:outline-2 focus:outline-solid focus:outline-accent'
const ACTIVE_CLASSES = 'bg-surface font-medium text-ink shadow-seg'
const INACTIVE_CLASSES = 'text-muted'
const DISABLED_FIELD_CLASSES =
  'flex h-9 items-center rounded-control border border-line bg-surface px-3 text-control text-faint opacity-60'
const ICON_LABEL = 'Ícone do projeto'
const ICON_BUTTON_CLASSES =
  'grid size-10 place-items-center rounded-control border focus:outline-2 focus:outline-solid focus:outline-accent'
const ICON_ACTIVE_CLASSES = 'border-accent bg-accent-soft text-accent'
const ICON_INACTIVE_CLASSES = 'border-line text-muted'
const FOOTER_CLASSES ='flex items-center justify-end gap-2'

const wizard = useNovoProjeto()
const { originKind, path, repo, mode, parentDir, branch, linkedRepo, name, symbol, description, busy, error } =
  storeToRefs(wizard)

function errorFor(field: string): string | undefined {
  return error.value?.field === field ? error.value.message : undefined
}
</script>

<template>
  <div class="flex flex-col gap-5.5">
    <div role="group" aria-label="Origem" :class="GROUP_CLASSES">
      <button
        v-for="option in ORIGIN_OPTIONS"
        :key="option.value"
        type="button"
        :data-testid="option.testid"
        :aria-pressed="originKind === option.value"
        :class="[OPTION_CLASSES, originKind === option.value ? ACTIVE_CLASSES : INACTIVE_CLASSES]"
        @click="originKind = option.value"
      >
        {{ option.label }}
      </button>
    </div>

    <template v-if="originKind === 'folder'">
      <div class="flex items-end gap-2">
        <UiField v-model="path" class="min-w-0 flex-1" label="Pasta do projeto" :error="errorFor('path')" data-testid="field-path" />
        <UiButton variant="quiet" size="lg" data-testid="pick-folder" @press="wizard.choosePath">
          {{ PICK_FOLDER_LABEL }}
        </UiButton>
      </div>
      <p v-if="linkedRepo" data-testid="linked-repo" class="text-caption text-muted">{{ LINKED_REPO_LABEL }} · {{ linkedRepo }}</p>
    </template>
    <template v-else>
      <UiField v-model="repo" label="Repositório" hint="dono/repo ou link" :error="errorFor('repo')" data-testid="field-repo" />
      <div role="group" aria-label="Modo" :class="GROUP_CLASSES">
        <button
          v-for="option in MODE_OPTIONS"
          :key="option.value"
          type="button"
          :data-testid="option.testid"
          :aria-pressed="mode === option.value"
          :class="[OPTION_CLASSES, mode === option.value ? ACTIVE_CLASSES : INACTIVE_CLASSES]"
          @click="mode = option.value"
        >
          {{ option.label }}
        </button>
      </div>
      <div class="flex items-end gap-2">
        <UiField v-model="parentDir" class="min-w-0 flex-1" label="Clonar em" :error="errorFor('parentDir')" data-testid="field-parent" />
        <UiButton variant="quiet" size="lg" @press="wizard.chooseParent">{{ PICK_FOLDER_LABEL }}</UiButton>
      </div>
      <UiField v-model="branch" label="Branch · opcional" :error="errorFor('branch')" data-testid="field-branch" />
      <div class="flex flex-col gap-1.5" aria-disabled="true">
        <span class="text-label font-medium text-muted">{{ START_FROM_LABEL }}</span>
        <div :class="DISABLED_FIELD_CLASSES">{{ START_FROM_VALUE }}</div>
      </div>
    </template>

    <UiField
      :model-value="name"
      label="Nome"
      data-testid="field-name"
      @update:model-value="wizard.setName"
    />
    <div class="flex flex-col gap-1.5">
      <span id="project-icon-label" class="text-label font-medium text-muted">{{ ICON_LABEL }}</span>
      <div role="radiogroup" aria-labelledby="project-icon-label" class="flex flex-wrap gap-2" data-testid="field-symbol">
        <button
          v-for="key in PROJECT_ICON_KEYS"
          :key="key"
          type="button"
          role="radio"
          :aria-checked="symbol === key"
          :aria-label="PROJECT_ICONS[key].label"
          :title="PROJECT_ICONS[key].label"
          :data-testid="`project-icon-${key}`"
          :class="[ICON_BUTTON_CLASSES, symbol === key ? ICON_ACTIVE_CLASSES : ICON_INACTIVE_CLASSES]"
          @click="wizard.setSymbol(key)"
        >
          <ProjectIcon :icon="key" class="size-5" />
        </button>
      </div>
      <p v-if="errorFor('symbol')" role="alert" class="text-small text-danger">{{ errorFor('symbol') }}</p>
    </div>
    <UiField v-model="description" label="Descrição · opcional" data-testid="field-description" />

    <p v-if="errorFor('general')" role="alert" class="text-small text-danger">{{ errorFor('general') }}</p>

    <div :class="FOOTER_CLASSES">
      <UiButton variant="quiet" size="lg" data-testid="step-back" @press="wizard.back">
        {{ CANCEL_LABEL }}
      </UiButton>
      <UiButton variant="primary" size="lg" :disabled="busy" data-testid="step-choose" @press="wizard.inspect">
        {{ CHOOSE_LABEL }}
      </UiButton>
    </div>
  </div>
</template>
