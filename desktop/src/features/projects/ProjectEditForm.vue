<script setup lang="ts">
import { ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiField from '../../ui/UiField.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import ProjectIconPicker from './ProjectIconPicker.vue'
import { CANCEL_LABEL, EMPTY_PATH_TEXT, SAVE_LABEL } from './projetosSample'
import type { PermissionLevel, Project, ProjectEdit } from './projectsApi'

const props = defineProps<{ project: Project }>()
const emit = defineEmits<{ save: [input: ProjectEdit, level: PermissionLevel]; cancel: [] }>()

const PATH_LABEL = 'Pasta'
const PERMISSION_LABEL = 'Permissões'
const PERMISSION_OPTIONS: { label: string; value: PermissionLevel }[] = [
  { label: 'Liberado', value: 'liberado' },
  { label: 'Edição', value: 'edicao' },
  { label: 'Leitura', value: 'leitura' },
]
const PERMISSION_HINTS: Record<PermissionLevel, string> = {
  liberado: 'Lê, edita e roda os comandos da lista segura.',
  edicao: 'Lê e edita arquivos, sem rodar comandos.',
  leitura: 'Só lê os arquivos da pasta.',
}
const CARD_CLASSES = 'flex min-w-0 flex-col gap-4 rounded-[26px] border border-line bg-surface p-5.5'
const PATH_CLASSES =
  'flex h-9 items-center truncate rounded-control border border-line bg-surface px-3 font-mono text-[11.5px] text-faint opacity-60'
const FOOTER_CLASSES = 'flex items-center justify-end gap-2.5'

const name = ref(props.project.name)
const symbol = ref(props.project.symbol)
const description = ref(props.project.description)
const level = ref<PermissionLevel>(props.project.permissionLevel)

function submit(): void {
  emit(
    'save',
    { name: name.value, symbol: symbol.value, description: description.value },
    level.value,
  )
}
</script>

<template>
  <div :class="CARD_CLASSES" data-testid="project-edit-form">
    <UiField v-model="name" label="Nome" data-testid="edit-name" />
    <div class="flex flex-col gap-1.5">
      <span class="text-label font-medium text-muted">{{ PATH_LABEL }}</span>
      <div :class="PATH_CLASSES" :title="project.path">{{ project.path || EMPTY_PATH_TEXT }}</div>
    </div>
    <ProjectIconPicker v-model="symbol" />
    <UiField v-model="description" label="Descrição" data-testid="edit-description" />
    <div class="flex flex-col gap-1.5">
      <UiSegmented
        v-model="level"
        :options="PERMISSION_OPTIONS"
        :label="PERMISSION_LABEL"
        data-testid="edit-permission"
      />
      <p class="text-caption text-muted">{{ PERMISSION_HINTS[level] }}</p>
    </div>
    <div :class="FOOTER_CLASSES">
      <UiButton variant="quiet" size="lg" data-testid="edit-cancel" @press="emit('cancel')">
        {{ CANCEL_LABEL }}
      </UiButton>
      <UiButton variant="primary" size="lg" data-testid="edit-save" @press="submit">
        {{ SAVE_LABEL }}
      </UiButton>
    </div>
  </div>
</template>
