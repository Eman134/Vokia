<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import UiField from '../../ui/UiField.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import { handReveal } from '../../ui/handDrawn'
import {
  SEGMENTED_ACTIVE_CLASSES,
  SEGMENTED_GROUP_CLASSES,
  SEGMENTED_INACTIVE_CLASSES,
  SEGMENTED_MARK_CLASSES,
  SEGMENTED_OPTION_CLASSES,
} from '../../ui/UiSegmented.vue'
import { storeToRefs } from 'pinia'
import {
  CANCEL_LABEL,
  CHOOSE_LABEL,
  LINKED_REPO_LABEL,
  PICK_FOLDER_LABEL,
} from './projetosCopy'
import ProjectColorPicker from './ProjectColorPicker.vue'
import ProjectIconPicker from './ProjectIconPicker.vue'
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

const GROUP_CLASSES = `${SEGMENTED_GROUP_CLASSES} w-fit`
const OPTION_CLASSES = `cursor-pointer ${SEGMENTED_OPTION_CLASSES}`
const FOOTER_CLASSES ='flex items-center justify-end gap-2'

const wizard = useNovoProjeto()
const { originKind, path, repo, mode, parentDir, branch, linkedRepo, name, symbol, color, description, busy, error } =
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
        :class="[OPTION_CLASSES, originKind === option.value ? SEGMENTED_ACTIVE_CLASSES : SEGMENTED_INACTIVE_CLASSES]"
        @click="originKind = option.value"
      >
        {{ option.label }}
        <UiHandLine :class="[SEGMENTED_MARK_CLASSES, handReveal(originKind === option.value)]" />
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
          :class="[OPTION_CLASSES, mode === option.value ? SEGMENTED_ACTIVE_CLASSES : SEGMENTED_INACTIVE_CLASSES]"
          @click="mode = option.value"
        >
          {{ option.label }}
          <UiHandLine :class="[SEGMENTED_MARK_CLASSES, handReveal(mode === option.value)]" />
        </button>
      </div>
      <div class="flex items-end gap-2">
        <UiField v-model="parentDir" class="min-w-0 flex-1" label="Clonar em" :error="errorFor('parentDir')" data-testid="field-parent" />
        <UiButton variant="quiet" size="lg" @press="wizard.chooseParent">{{ PICK_FOLDER_LABEL }}</UiButton>
      </div>
      <UiField v-model="branch" label="Branch · opcional" :error="errorFor('branch')" data-testid="field-branch" />
    </template>

    <UiField
      :model-value="name"
      label="Nome"
      data-testid="field-name"
      @update:model-value="wizard.setName"
    />
    <div class="flex flex-col gap-1.5">
      <ProjectIconPicker :model-value="symbol" @update:model-value="wizard.setSymbol" />
      <p v-if="errorFor('symbol')" role="alert" class="text-small text-danger">{{ errorFor('symbol') }}</p>
    </div>
    <ProjectColorPicker v-model="color" />
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
