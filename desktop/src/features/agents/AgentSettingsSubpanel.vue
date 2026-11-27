<script setup lang="ts">
import { computed, nextTick, onMounted, ref, useId, watch } from 'vue'
import { useNavigation } from '../../shell/useNavigation'
import { trapFocus } from '../backlog/dialogFocus'
import UiButton from '../../ui/UiButton.vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiField from '../../ui/UiField.vue'
import UiNotice from '../../ui/UiNotice.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import UiSelect from '../../ui/UiSelect.vue'
import UiSwitch from '../../ui/UiSwitch.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import { useArtifacts } from '../artifacts/useArtifacts'
import AvatarPicker from '../identities/AvatarPicker.vue'
import { useLearningSuggestions } from '../memory/useLearningSuggestions'
import { EXECUTION_MODE_INFO } from '../permissions/executionModes'
import { useExecutionMode } from '../permissions/useExecutionMode'
import { useProjects } from '../projects/useProjects'
import type {
  AgentBackupMode,
  AgentNotificationMode,
  AgentProfile,
  LlmTarget,
  ModelTier,
} from '../team/teamApi'
import { useRoster } from '../team/useRoster'
import AgentInstructions from './AgentInstructions.vue'
import { AGENT_CHAT_COLORS, findChatColor } from './agentChatColors'
import {
  BUILTIN_TEXT,
  COMMAND_LABEL,
  DELETE_AGENT_CONFIRM_LABEL,
  DELETE_AGENT_MESSAGE,
  DELETE_LABEL,
  NOTIFICATION_NOTE,
  RESTORE_DEFAULT_LABEL,
} from './agentesCopy'

export type AgentSubpanelId =
  | 'avatar'
  | 'backup'
  | 'chatColor'
  | 'instructions'
  | 'memories'
  | 'skills'
  | 'files'
  | 'artifacts'
  | 'routines'
  | 'mcp'
  | 'export'
  | 'permissions'
  | 'folders'
  | 'advanced'
  | 'notifications'

const SUBPANEL_TITLES: Record<AgentSubpanelId, string> = {
  avatar: 'Avatar do agente',
  backup: 'Modelos de reserva',
  chatColor: 'Cor do chat',
  instructions: 'Instruções',
  memories: 'Memórias',
  skills: 'Habilidades',
  files: 'Arquivos',
  artifacts: 'Artefatos',
  routines: 'Rotinas',
  mcp: 'Apps / MCP',
  export: 'Exportar / Compartilhar agente',
  permissions: 'Permissões',
  folders: 'Pastas permitidas',
  advanced: 'Avançado',
  notifications: 'Me notificar',
}

const BACKUP_MODE_OPTIONS: { label: string; value: AgentBackupMode }[] = [
  { label: 'Fallback global', value: 'global' },
  { label: 'Perguntar antes', value: 'ask' },
  { label: 'Automático', value: 'auto' },
]

const NOTIFICATION_MODE_OPTIONS: { label: string; value: AgentNotificationMode; hint: string }[] = [
  {
    label: 'Padrão do app (Todas)',
    value: 'default',
    hint: 'Segue a configuração global de notificações do Vokia.',
  },
  {
    label: 'Só quando precisar de mim',
    value: 'needs_me',
    hint: 'Notifica apenas quando abrir uma decisão, aprovação ou pergunta.',
  },
  {
    label: 'Silenciar',
    value: 'mute',
    hint: 'Não dispara notificações nativas mesmo com turnos concluídos em segundo plano.',
  },
]

const TIER_OPTIONS: { label: string; value: ModelTier }[] = [
  { label: 'Básico', value: 'basic' },
  { label: 'Intermediário', value: 'intermediate' },
  { label: 'Avançado', value: 'advanced' },
]

const props = defineProps<{
  agent: AgentProfile
  subpanel: AgentSubpanelId
  saving: boolean
  removing: boolean
  failure: string | null
}>()

const emit = defineEmits<{
  back: []
  patch: [
    changes: Partial<{
      avatar: string
      backupMode: AgentBackupMode | null
      fallback: LlmTarget[]
      chatColor: string | null
      extraDirs: string[]
      command: string
      followDefaultLlm: boolean
      notificationMode: AgentNotificationMode | null
    }>,
  ]
  remove: []
  clearFailure: []
}>()

const roster = useRoster()
const navigation = useNavigation()
const projects = useProjects()
const learning = useLearningSuggestions()
const artifacts = useArtifacts()
const executionMode = useExecutionMode()

const dialogRef = ref<HTMLElement>()
const closeButtonRef = ref<{ $el: HTMLElement } | null>(null)
const titleId = useId()
const avatarLabelId = useId()

const newDir = ref('')
const newBackupCli = ref(roster.presets[0]?.id ?? '')
const newBackupTier = ref<ModelTier>('intermediate')
const commandDraft = ref(props.agent.command)
const copiedExport = ref(false)

watch(
  () => props.agent.command,
  (val) => {
    commandDraft.value = val
  },
)

onMounted(() => {
  void nextTick(() => {
    closeButtonRef.value?.$el.focus()
  })
})

const avatarOwners = computed<Record<string, string>>(() =>
  Object.fromEntries(
    roster.agents.filter((a) => a.id !== props.agent.id).map((a) => [a.avatar, a.name]),
  ),
)

const avatarModel = computed<string>({
  get: () => props.agent.avatar,
  set: (next) => {
    if (!props.agent.builtin && next !== props.agent.avatar) {
      emit('patch', { avatar: next })
    }
  },
})

const currentPreset = computed(() => roster.presets.find((p) => p.id === props.agent.cli))
const providerOptions = computed(() =>
  roster.presets.map((p) => ({ label: p.label, value: p.id })),
)

const currentTransportLabel = computed(() => {
  const preset = currentPreset.value
  const transport = preset?.toolTransport ?? (preset?.supportsMcp ? 'mcp-native' : 'none')
  if (transport === 'mcp-native') return 'MCP nativo'
  if (transport === 'vokia-cli') return 'Vokia CLI'
  return 'Só texto'
})

const currentPermissionLabel = computed(() => {
  const mode = executionMode.modeOf(projects.selected?.id)
  return EXECUTION_MODE_INFO[mode]?.label ?? 'Leitura'
})

const exportedProfileJson = computed(() => {
  const payload = {
    name: props.agent.name,
    role: props.agent.role,
    title: props.agent.title ?? null,
    description: props.agent.description ?? null,
    avatar: props.agent.avatar,
    chatColor: props.agent.chatColor ?? null,
    cli: props.agent.cli,
    model: props.agent.model,
    modelTier: props.agent.modelTier ?? null,
    reasoningEffort: props.agent.reasoningEffort ?? null,
    followDefaultLlm: props.agent.followDefaultLlm,
    fallback: props.agent.fallback,
    extraInstructions: props.agent.extraInstructions,
    extraDirs: props.agent.extraDirs ?? [],
    notificationMode: props.agent.notificationMode ?? 'default',
    backupMode: props.agent.backupMode ?? 'global',
  }
  return JSON.stringify(payload, null, 2)
})

function addExtraDir(): void {
  const trimmed = newDir.value.trim()
  if (!trimmed) return
  const current = props.agent.extraDirs ?? []
  if (current.includes(trimmed)) {
    newDir.value = ''
    return
  }
  emit('patch', { extraDirs: [...current, trimmed] })
  newDir.value = ''
}

function removeExtraDir(index: number): void {
  const next = [...(props.agent.extraDirs ?? [])]
  next.splice(index, 1)
  emit('patch', { extraDirs: next })
}

function addBackupTarget(): void {
  if (!newBackupCli.value) return
  const targetPreset = roster.presets.find((p) => p.id === newBackupCli.value)
  const targetModel =
    targetPreset?.tiers?.[newBackupTier.value]?.trim() || targetPreset?.defaultModel || ''
  const next: LlmTarget[] = [
    ...props.agent.fallback,
    { cli: newBackupCli.value, modelTier: newBackupTier.value, model: targetModel },
  ]
  emit('patch', { fallback: next })
}

function removeBackupTarget(index: number): void {
  const next = [...props.agent.fallback]
  next.splice(index, 1)
  emit('patch', { fallback: next })
}

function moveBackupTarget(index: number, delta: -1 | 1): void {
  const target = index + delta
  if (target < 0 || target >= props.agent.fallback.length) return
  const next = [...props.agent.fallback]
  const [item] = next.splice(index, 1)
  next.splice(target, 0, item)
  emit('patch', { fallback: next })
}

function saveCommand(): void {
  const trimmed = commandDraft.value.trim()
  if (!trimmed || trimmed === props.agent.command) return
  emit('patch', { command: trimmed })
}

function restoreDefaultCommand(): void {
  const def = currentPreset.value?.defaultCommand
  if (!def) return
  commandDraft.value = def
  if (def !== props.agent.command) {
    emit('patch', { command: def })
  }
}

async function copyExportJson(): Promise<void> {
  try {
    await navigator.clipboard.writeText(exportedProfileJson.value)
    copiedExport.value = true
    setTimeout(() => {
      copiedExport.value = false
    }, 2000)
  } catch {
    // clipboard unavailable
  }
}

function downloadExportJson(): void {
  const blob = new Blob([exportedProfileJson.value], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = `${props.agent.id}-agent.json`
  link.click()
  URL.revokeObjectURL(url)
}
</script>

<template>
  <Teleport to="body">
    <div
      class="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4"
      @click.self="emit('back')"
    >
      <section
        ref="dialogRef"
        data-testid="agent-subpanel-dialog"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="titleId"
        tabindex="-1"
        class="flex max-h-[82vh] w-[560px] max-w-full flex-col overflow-hidden rounded-md border border-line bg-surface shadow-paper focus:outline-none"
        @keydown.esc.stop="emit('back')"
        @keydown.tab="trapFocus($event, dialogRef)"
      >
        <header class="flex shrink-0 items-center justify-between gap-3 border-b border-line px-4 py-3">
          <div class="flex min-w-0 items-center gap-2">
            <h2 :id="titleId" class="truncate text-control font-bold text-ink">
              {{ SUBPANEL_TITLES[subpanel] }}
            </h2>
            <span class="truncate text-caption text-muted">· {{ agent.name }}</span>
          </div>
          <UiCloseButton ref="closeButtonRef" data-testid="agent-subpanel-close" @press="emit('back')" />
        </header>

        <div class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-5">
          <UiNotice v-if="failure !== null" :text="failure" @close="emit('clearFailure')" />

    <!-- Avatar -->
    <div v-if="subpanel === 'avatar'" class="flex flex-col gap-3">
      <p v-if="agent.builtin" class="text-caption text-muted">{{ BUILTIN_TEXT }}</p>
      <div v-else class="flex flex-col gap-2">
        <span :id="avatarLabelId" class="text-label font-medium text-muted">Escolha um retrato</span>
        <AvatarPicker v-model="avatarModel" :label-id="avatarLabelId" :unavailable="avatarOwners" />
      </div>
    </div>

    <!-- Modelos de reserva -->
    <div v-else-if="subpanel === 'backup'" class="flex flex-col gap-4">
      <div class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">Política de failover</span>
        <UiSegmented
          label="Política de failover"
          :model-value="agent.backupMode ?? 'global'"
          :options="BACKUP_MODE_OPTIONS"
          :disabled="saving"
          @update:model-value="emit('patch', { backupMode: $event as AgentBackupMode })"
        />
      </div>

      <div class="flex flex-col gap-2">
        <span class="text-label font-medium text-muted">Ordem de modelos de reserva</span>
        <p v-if="agent.fallback.length === 0" class="text-caption text-muted">
          Nenhum modelo de reserva específico. Usando fallback global quando necessário.
        </p>
        <ul v-else class="flex flex-col gap-1.5">
          <li
            v-for="(item, index) in agent.fallback"
            :key="`${item.cli}-${index}`"
            class="flex items-center justify-between gap-2 rounded-md border border-line bg-surface-2 px-3 py-2 text-caption"
          >
            <span class="truncate font-medium text-ink">
              {{ index + 1 }}. {{ item.cli }} · {{ item.model || item.modelTier || 'padrão' }}
            </span>
            <span class="flex items-center gap-1">
              <button
                type="button"
                class="cursor-pointer rounded-sm px-1.5 py-0.5 text-small text-muted hover:bg-surface hover:text-ink disabled:cursor-not-allowed disabled:opacity-40"
                :class="INK_FOCUS_CLASSES"
                :disabled="index === 0 || saving || agent.builtin"
                aria-label="Mover para cima"
                @click="moveBackupTarget(index, -1)"
              >
                &uarr;
              </button>
              <button
                type="button"
                class="cursor-pointer rounded-sm px-1.5 py-0.5 text-small text-muted hover:bg-surface hover:text-ink disabled:cursor-not-allowed disabled:opacity-40"
                :class="INK_FOCUS_CLASSES"
                :disabled="index === agent.fallback.length - 1 || saving || agent.builtin"
                aria-label="Mover para baixo"
                @click="moveBackupTarget(index, 1)"
              >
                &darr;
              </button>
              <button
                type="button"
                class="cursor-pointer rounded-sm px-1.5 py-0.5 text-small text-danger hover:bg-surface disabled:cursor-not-allowed disabled:opacity-40"
                :class="INK_FOCUS_CLASSES"
                :disabled="saving || agent.builtin"
                aria-label="Remover modelo de reserva"
                @click="removeBackupTarget(index)"
              >
                &times;
              </button>
            </span>
          </li>
        </ul>

        <div v-if="!agent.builtin" class="flex flex-wrap items-end gap-2 pt-1">
          <label class="flex min-w-28 flex-1 flex-col gap-1">
            <span class="text-small text-muted">Provedor</span>
            <UiSelect v-model="newBackupCli" :options="providerOptions" label="Provedor reserva" />
          </label>
          <label class="flex min-w-28 flex-1 flex-col gap-1">
            <span class="text-small text-muted">Nível</span>
            <UiSelect v-model="newBackupTier" :options="TIER_OPTIONS" label="Nível reserva" />
          </label>
          <UiButton variant="quiet" size="sm" :disabled="saving" @press="addBackupTarget()">
            Adicionar
          </UiButton>
        </div>
      </div>
    </div>

    <!-- Cor do chat -->
    <div v-else-if="subpanel === 'chatColor'" class="flex flex-col gap-3">
      <p class="text-caption text-muted">
        Destaque opcional do agente nas conversas, adaptado aos temas claro e escuro.
      </p>
      <div class="grid grid-cols-2 gap-2">
        <button
          type="button"
          class="flex cursor-pointer items-center gap-2.5 rounded-md border px-3 py-2 text-left text-caption"
          :class="[
            INK_FOCUS_CLASSES,
            !agent.chatColor ? 'border-accent bg-accent-soft font-semibold text-ink' : 'border-line bg-surface hover:bg-surface-2 text-ink',
          ]"
          :disabled="agent.builtin || saving"
          @click="emit('patch', { chatColor: null })"
        >
          <span class="size-3.5 rounded-full border border-line bg-surface-2" aria-hidden="true" />
          <span>Nenhuma</span>
        </button>
        <button
          v-for="color in AGENT_CHAT_COLORS"
          :key="color.id"
          type="button"
          class="flex cursor-pointer items-center gap-2.5 rounded-md border px-3 py-2 text-left text-caption"
          :class="[
            INK_FOCUS_CLASSES,
            findChatColor(agent.chatColor)?.id === color.id
              ? 'border-accent bg-accent-soft font-semibold text-ink'
              : 'border-line bg-surface hover:bg-surface-2 text-ink',
          ]"
          :disabled="agent.builtin || saving"
          @click="emit('patch', { chatColor: color.id })"
        >
          <span class="size-3.5 shrink-0 rounded-full border border-project-line" :class="color.swatchClass" aria-hidden="true" />
          <span class="truncate">{{ color.label }}</span>
        </button>
      </div>
    </div>

    <!-- Instruções -->
    <div v-else-if="subpanel === 'instructions'" class="flex flex-col gap-3">
      <div class="rounded-md border border-line bg-surface-2 px-3 py-2 text-caption text-muted">
        Papel atual: <strong class="font-semibold text-ink">{{ agent.role }}</strong>
      </div>
      <AgentInstructions :key="agent.id" :agent="agent" />
    </div>

    <!-- Memórias -->
    <div v-else-if="subpanel === 'memories'" class="flex flex-col gap-3">
      <p class="text-caption text-muted">
        {{ learning.memories.length }} memória(s) salva(s) no projeto ativo ·
        {{ learning.pendingMemoryCount }} sugestão(ões) pendente(s).
      </p>
      <ul v-if="learning.memories.length > 0" class="flex max-h-56 flex-col gap-1.5 overflow-y-auto">
        <li
          v-for="mem in learning.memories.slice(0, 8)"
          :key="mem.id"
          class="rounded-md border border-line bg-surface-2 px-3 py-2 text-caption text-ink"
        >
          <p class="font-medium">{{ mem.fact }}</p>
          <p v-if="mem.source" class="mt-0.5 truncate text-small text-muted">{{ mem.source }}</p>
        </li>
      </ul>
      <div class="flex justify-end">
        <UiButton variant="quiet" size="sm" @press="navigation.goTo('memoria')">
          Abrir Memória
        </UiButton>
      </div>
    </div>

    <!-- Habilidades -->
    <div v-else-if="subpanel === 'skills'" class="flex flex-col gap-3">
      <p class="text-caption text-muted">
        Gerencie as habilidades (`SKILL.md`) disponíveis para o time e para este agente.
      </p>
      <div class="flex justify-end">
        <UiButton variant="quiet" size="sm" @press="navigation.goTo('skills')">
          Abrir Habilidades
        </UiButton>
      </div>
    </div>

    <!-- Arquivos -->
    <div v-else-if="subpanel === 'files'" class="flex flex-col gap-3">
      <p class="text-caption text-muted">
        Nenhum arquivo de referência anexado diretamente a este agente. Anexos de conversa ficam na pasta do projeto ativo.
      </p>
    </div>

    <!-- Artefatos -->
    <div v-else-if="subpanel === 'artifacts'" class="flex flex-col gap-3">
      <p class="text-caption text-muted">
        {{ artifacts.documents.length }} artefato(s) no projeto ativo.
      </p>
      <ul v-if="artifacts.documents.length > 0" class="flex max-h-56 flex-col gap-1.5 overflow-y-auto">
        <li
          v-for="doc in artifacts.documents.slice(0, 8)"
          :key="doc.id"
          class="flex items-center justify-between gap-2 rounded-md border border-line bg-surface-2 px-3 py-2 text-caption"
        >
          <span class="truncate font-medium text-ink">{{ doc.title }}</span>
          <span class="shrink-0 text-small text-muted">{{ doc.kind }}</span>
        </li>
      </ul>
      <div class="flex justify-end">
        <UiButton variant="quiet" size="sm" @press="navigation.goTo('canvas')">
          Abrir Artefatos
        </UiButton>
      </div>
    </div>

    <!-- Rotinas -->
    <div v-else-if="subpanel === 'routines'" class="flex flex-col gap-3">
      <div class="rounded-md border border-line bg-surface-2 px-3.5 py-3 text-caption text-muted">
        <p class="font-medium text-ink">0 configuradas</p>
        <p class="mt-1">
          Agendamentos recorrentes e gatilhos de repositório para este agente aparecerão aqui quando habilitados.
        </p>
      </div>
    </div>

    <!-- Apps / MCP -->
    <div v-else-if="subpanel === 'mcp'" class="flex flex-col gap-3">
      <div class="rounded-md border border-line bg-surface-2 px-3.5 py-3 text-caption">
        <p class="font-medium text-ink">Transporte ativo: {{ currentTransportLabel }}</p>
        <p class="mt-1 text-muted">
          Provedor: {{ currentPreset?.label ?? agent.cli }}. Todas as ferramentas MCP do projeto ficam disponíveis quando o transporte suportar ferramentas.
        </p>
      </div>
    </div>

    <!-- Exportar / Compartilhar -->
    <div v-else-if="subpanel === 'export'" class="flex flex-col gap-3">
      <p class="text-caption text-muted">
        Perfil exportável em JSON sem credenciais nem tokens.
      </p>
      <pre class="max-h-56 overflow-auto rounded-md border border-line bg-surface-2 p-3 font-mono text-small text-ink">{{ exportedProfileJson }}</pre>
      <div class="flex justify-end gap-2">
        <UiButton variant="quiet" size="sm" @press="copyExportJson()">
          {{ copiedExport ? 'Copiado' : 'Copiar JSON' }}
        </UiButton>
        <UiButton variant="primary" size="sm" @press="downloadExportJson()">
          Baixar JSON
        </UiButton>
      </div>
    </div>

    <!-- Permissões -->
    <div v-else-if="subpanel === 'permissions'" class="flex flex-col gap-3">
      <div class="rounded-md border border-line bg-surface-2 px-3.5 py-3 text-caption">
        <p class="font-medium text-ink">Modo do projeto: {{ currentPermissionLabel }}</p>
        <p class="mt-1 text-muted">
          Aprovação e bloqueio de comandos são gerenciados por projeto e nas regras globais.
        </p>
      </div>
      <div class="flex justify-end">
        <UiButton variant="quiet" size="sm" @press="navigation.openSettingsSection('Permissões')">
          Configurar permissões
        </UiButton>
      </div>
    </div>

    <!-- Pastas -->
    <div v-else-if="subpanel === 'folders'" class="flex flex-col gap-3">
      <p class="text-caption text-muted">
        Diretórios adicionais passados como <code class="font-mono text-ink">--add-dir</code> ao iniciar turnos deste agente.
      </p>
      <ul v-if="(agent.extraDirs ?? []).length > 0" class="flex flex-col gap-1.5">
        <li
          v-for="(dir, index) in agent.extraDirs"
          :key="`${dir}-${index}`"
          class="flex items-center justify-between gap-2 rounded-md border border-line bg-surface-2 px-3 py-2 text-caption"
        >
          <span class="truncate font-mono text-ink">{{ dir }}</span>
          <button
            type="button"
            class="cursor-pointer rounded-sm px-1.5 py-0.5 text-small text-danger hover:bg-surface disabled:cursor-not-allowed disabled:opacity-40"
            :class="INK_FOCUS_CLASSES"
            :disabled="saving || agent.builtin"
            aria-label="Remover pasta"
            @click="removeExtraDir(index)"
          >
            &times;
          </button>
        </li>
      </ul>
      <p v-else class="text-caption text-muted">Nenhuma pasta adicional configurada.</p>

      <div v-if="!agent.builtin" class="flex items-end gap-2">
        <UiField
          v-model="newDir"
          class="min-w-0 flex-1"
          label="Adicionar diretório"
          placeholder="C:\caminho\adicional"
          @keydown.enter.prevent="addExtraDir()"
        />
        <UiButton variant="quiet" size="sm" :disabled="saving || !newDir.trim()" @press="addExtraDir()">
          Adicionar
        </UiButton>
      </div>
    </div>

    <!-- Avançado -->
    <div v-else-if="subpanel === 'advanced'" class="flex flex-col gap-4">
      <div class="flex items-center justify-between gap-3 rounded-md border border-line bg-surface-2 px-3.5 py-2.5">
        <div class="min-w-0">
          <p class="text-control font-medium text-ink">Seguir LLM global</p>
          <p class="text-small text-muted">Usa o provedor/modelo ativo globalmente como primeira opção.</p>
        </div>
        <UiSwitch
          label="Seguir LLM global"
          :model-value="agent.followDefaultLlm"
          :disabled="saving || agent.builtin"
          @update:model-value="emit('patch', { followDefaultLlm: $event })"
        />
      </div>

      <div class="flex flex-col gap-1.5">
        <div class="flex items-center justify-between gap-2">
          <span class="text-label font-medium text-muted">{{ COMMAND_LABEL }}</span>
          <button
            v-if="!agent.builtin && currentPreset?.defaultCommand"
            type="button"
            class="cursor-pointer text-caption text-accent hover:underline"
            :class="INK_FOCUS_CLASSES"
            @click="restoreDefaultCommand()"
          >
            {{ RESTORE_DEFAULT_LABEL }}
          </button>
        </div>
        <div class="flex items-end gap-2">
          <UiField
            v-model="commandDraft"
            class="min-w-0 flex-1"
            :label="COMMAND_LABEL"
            :disabled="agent.builtin || saving"
          />
          <UiButton
            v-if="!agent.builtin"
            variant="quiet"
            size="sm"
            :disabled="saving || !commandDraft.trim() || commandDraft.trim() === agent.command"
            @press="saveCommand()"
          >
            Salvar
          </UiButton>
        </div>
      </div>

      <div class="rounded-md border border-line bg-surface-2 px-3.5 py-2.5 text-caption text-muted">
        <p class="font-medium text-ink">Quando ocupado: enfileirar (queue) ou corrigir (steer)</p>
        <p class="mt-1">
          Pedidos enviados durante um turno ativo entram na fila por padrão; você pode enviar correção imediata (steer) na barra do compositor.
        </p>
      </div>

      <div v-if="!agent.builtin" class="mt-2 flex items-center justify-between border-t border-line pt-4">
        <span class="text-caption text-muted">Zona de exclusão</span>
        <UiConfirm
          :label="DELETE_LABEL"
          :message="DELETE_AGENT_MESSAGE"
          :confirm-label="DELETE_AGENT_CONFIRM_LABEL"
          :busy="removing"
          @confirm="emit('remove')"
        />
      </div>
    </div>

    <!-- Me notificar -->
    <div v-else-if="subpanel === 'notifications'" class="flex flex-col gap-3">
      <div class="flex flex-col gap-1.5">
        <button
          v-for="opt in NOTIFICATION_MODE_OPTIONS"
          :key="opt.value"
          type="button"
          class="flex cursor-pointer flex-col gap-0.5 rounded-md border px-3.5 py-2.5 text-left"
          :class="[
            INK_FOCUS_CLASSES,
            (agent.notificationMode ?? 'default') === opt.value
              ? 'border-accent bg-accent-soft'
              : 'border-line bg-surface hover:bg-surface-2',
          ]"
          :disabled="saving || agent.builtin"
          @click="emit('patch', { notificationMode: opt.value })"
        >
          <span class="text-control font-medium text-ink">{{ opt.label }}</span>
          <span class="text-small text-muted">{{ opt.hint }}</span>
        </button>
      </div>
      <p class="text-caption text-muted">{{ NOTIFICATION_NOTE }}</p>
    </div>
        </div>
      </section>
    </div>
  </Teleport>
</template>
