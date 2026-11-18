<script setup lang="ts">
import { computed, onMounted, ref, useId } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiField from '../../ui/UiField.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { trapFocus } from '../backlog/dialogFocus'
import ProjectColorPicker from './ProjectColorPicker.vue'
import ProjectIconPicker from './ProjectIconPicker.vue'
import { projectColorOf } from './projectColors'
import { CANCEL_LABEL, EMPTY_PATH_TEXT, SAVE_LABEL } from './projetosSample'
import { SETUP_COMMAND_MAX_CHARS, type PermissionLevel, type Project, type ProjectEdit } from './projectsApi'

const props = defineProps<{ project: Project }>()
const emit = defineEmits<{ save: [input: ProjectEdit, level: PermissionLevel]; cancel: []; remove: [] }>()

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
const DIALOG_TITLE = 'Editar projeto'
const NAME_FIELD_SELECTOR = 'input'
const OVERLAY_CLASSES = 'fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4'
const DIALOG_CLASSES =
  'flex max-h-full w-120 max-w-full min-w-0 flex-col gap-4 overflow-y-auto rounded-dialog border border-line bg-surface p-5.5 shadow-float'
const PATH_CLASSES =
  'flex h-9 items-center truncate rounded-control border border-line bg-surface px-3 font-mono text-[11.5px] text-faint opacity-60'
const FOOTER_CLASSES = 'flex flex-wrap items-center justify-end gap-2.5'

const titleId = useId()
const dialog = ref<HTMLElement>()
const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null

const name = ref(props.project.name)
const symbol = ref(props.project.symbol)
const description = ref(props.project.description)
const color = ref<string>(projectColorOf(props.project))
const level = ref<PermissionLevel>(props.project.permissionLevel)
const setupCommand = ref(props.project.setupCommand ?? '')
const SETUP_COMMAND_LABEL = 'Comando de preparo (opcional)'
const SETUP_COMMAND_HINT = 'Roda dentro do worktree de cada item; até 300 caracteres.'
const SETUP_COMMAND_TOO_LONG = `Use no máximo ${SETUP_COMMAND_MAX_CHARS} caracteres.`
const setupCommandError = computed(() =>
  [...setupCommand.value].length > SETUP_COMMAND_MAX_CHARS ? SETUP_COMMAND_TOO_LONG : undefined,
)

function close(): void {
  previousFocus?.focus()
  emit('cancel')
}

function submit(): void {
  if (setupCommandError.value) return
  emit(
    'save',
    {
      name: name.value,
      symbol: symbol.value,
      description: description.value,
      color: color.value,
      setupCommand: setupCommand.value,
    },
    level.value,
  )
}

onMounted(() => dialog.value?.querySelector<HTMLElement>(NAME_FIELD_SELECTOR)?.focus())
</script>

<template>
  <div :class="OVERLAY_CLASSES" @click.self="close()">
    <section
      ref="dialog"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      tabindex="-1"
      :class="DIALOG_CLASSES"
      data-testid="project-edit-form"
      @keydown.esc.stop="close()"
      @keydown.tab="trapFocus($event, dialog)"
    >
    <h2 :id="titleId" class="text-control font-bold text-ink">{{ DIALOG_TITLE }}</h2>
    <UiField v-model="name" label="Nome" data-testid="edit-name" />
    <div class="flex flex-col gap-1.5">
      <span class="text-label font-medium text-muted">{{ PATH_LABEL }}</span>
      <div :class="PATH_CLASSES" :title="project.path">{{ project.path || EMPTY_PATH_TEXT }}</div>
    </div>
    <ProjectIconPicker v-model="symbol" />
    <ProjectColorPicker v-model="color" />
    <UiField v-model="description" label="Descrição" data-testid="edit-description" />
    <UiField
      v-model="setupCommand"
      :label="SETUP_COMMAND_LABEL"
      :hint="SETUP_COMMAND_HINT"
      :error="setupCommandError"
      data-testid="edit-setup-command"
    />
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
      <UiConfirm
        class="mr-auto"
        label="Excluir"
        message="Remove o projeto, conversas, backlog e planos do app. A pasta no disco não é apagada."
        confirm-label="Excluir projeto"
        @confirm="emit('remove')"
      />
      <UiButton variant="quiet" size="lg" data-testid="edit-cancel" @press="close()">
        {{ CANCEL_LABEL }}
      </UiButton>
      <UiButton variant="primary" size="lg" data-testid="edit-save" :disabled="Boolean(setupCommandError)" @press="submit">
        {{ SAVE_LABEL }}
      </UiButton>
    </div>
    </section>
  </div>
</template>
