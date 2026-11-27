<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useNavigation } from '../../shell/useNavigation'
import UiButton from '../../ui/UiButton.vue'
import UiCreateButton from '../../ui/UiCreateButton.vue'
import UiField from '../../ui/UiField.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { HAND_SELECTED_CLASSES, INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import UiSketchBox from '../../ui/UiSketchBox.vue'
import { useArtifacts } from '../artifacts/useArtifacts'
import { useLearningSuggestions } from '../memory/useLearningSuggestions'
import { hashSeed } from '../orchestration/flowGraph'
import { EXECUTION_MODE_INFO } from '../permissions/executionModes'
import { useExecutionMode } from '../permissions/useExecutionMode'
import { useProjects } from '../projects/useProjects'
import ProviderModelPickerModal, {
  type ModelSelectionPayload,
} from '../providers/ProviderModelPickerModal.vue'
import { EFFORT_LABELS, normalizeReasoningEffort } from '../providers/providerModelCatalog'
import { useProviders } from '../providers/useProviders'
import { useTemporaryOverrides } from '../providers/useTemporaryOverrides'
import AgentPortrait from '../team/AgentPortrait.vue'
import type { AgentDraft, AgentProfile, ModelTier } from '../team/teamApi'
import { normalizeName, useRoster } from '../team/useRoster'
import { useUsageAnalytics } from '../usage/useUsageAnalytics'
import AgentSettingsRow from './AgentSettingsRow.vue'
import AgentSettingsSubpanel, { type AgentSubpanelId } from './AgentSettingsSubpanel.vue'
import NewAgentDialog from './NewAgentDialog.vue'
import TemporaryOverrideBadge from './TemporaryOverrideBadge.vue'
import { findChatColor } from './agentChatColors'
import {
  BUILTIN_TEXT,
  CONFIGURE_LLMS_LABEL,
  DESCRIPTION_PLACEHOLDER,
  NEW_AGENT_LABEL,
  NO_AGENTS_TEXT,
  NOTIFICATION_NOTE,
  SEARCH_LABEL,
  SECTION_BRAIN,
  SECTION_DOES,
  SECTION_KNOWS,
  SECTION_NOTIFY,
  SECTION_RULES,
  TITLE_PLACEHOLDER,
} from './agentesCopy'

const COLUMNS_CLASSES =
  'grid min-w-0 grid-cols-1 items-start gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(0,380px)] xl:grid-cols-[minmax(0,1fr)_minmax(0,440px)]'
const LIST_CLASSES = 'flex w-full min-w-0 flex-col gap-3 rounded-md bg-surface p-5'
const FICHA_CLASSES = 'flex w-full min-w-0 flex-col gap-4 rounded-md border border-line bg-surface p-5'
const SECTION_CARD_CLASSES =
  'flex flex-col divide-y divide-line overflow-hidden rounded-md border border-line bg-surface'
const SECTION_TITLE_CLASSES = 'px-1 text-small font-semibold uppercase tracking-wider text-muted'
const IDENTITY_INPUT_CLASSES =
  `w-full min-w-0 rounded-md border border-line bg-surface px-3 py-1.5 text-control text-ink placeholder:text-faint disabled:bg-surface-2 disabled:text-muted ${INK_FOCUS_CLASSES}`
const ROW_BASE_CLASSES =
  'flex cursor-pointer items-center gap-3 rounded-md border-[1.5px] px-3 py-2 text-left'
const ROW_ACTIVE_CLASSES = HAND_SELECTED_CLASSES
const ROW_INACTIVE_CLASSES = 'border-transparent hover:bg-surface-2'
const NAME_CLASSES = 'truncate text-control font-semibold text-ink'
const FRAME_CLASSES = 'shrink-0 p-0.5'
const PORTRAIT_SIZE = 32
const FICHA_PORTRAIT_SIZE = 64

const TIER_LABELS: Record<ModelTier, string> = {
  basic: 'Básico',
  intermediate: 'Intermediário',
  advanced: 'Avançado',
}

const roster = useRoster()
const providers = useProviders()
const overrides = useTemporaryOverrides()
const navigation = useNavigation()
const projects = useProjects()
const learning = useLearningSuggestions()
const artifacts = useArtifacts()
const executionMode = useExecutionMode()
const usageAnalytics = useUsageAnalytics()

const query = ref('')
const selectedId = ref<string | null>(null)
const dialog = ref<{ agent?: AgentProfile } | null>(null)
const modelPickerOpen = ref(false)
const activeSubpanel = ref<AgentSubpanelId | null>(null)
const subpanelTrigger = ref<HTMLElement | null>(null)
const failure = ref<string | null>(null)
const saving = ref(false)
const removing = ref(false)
const identitySaved = ref(false)

const draftName = ref('')
const draftTitle = ref('')
const draftDescription = ref('')
let identityTimer: ReturnType<typeof setTimeout> | null = null

const visible = computed(() => {
  const key = normalizeName(query.value.trim())
  return roster.agents.filter(
    (agent) =>
      key === '' ||
      normalizeName(agent.name).includes(key) ||
      normalizeName(agent.role).includes(key) ||
      normalizeName(agent.title ?? '').includes(key),
  )
})

const selected = computed(
  () => roster.agents.find((agent) => agent.id === selectedId.value) ?? roster.agents[0] ?? null,
)

watch(
  selected,
  (agent, prev) => {
    if (agent?.id !== prev?.id) {
      activeSubpanel.value = null
      failure.value = null
      if (identityTimer) {
        clearTimeout(identityTimer)
        identityTimer = null
      }
    }
    draftName.value = agent?.name ?? ''
    draftTitle.value = agent?.title ?? ''
    draftDescription.value = agent?.description ?? ''
  },
  { immediate: true },
)

const modelSummary = computed(() => {
  const agent = selected.value
  if (!agent) return ''
  const preset = roster.presets.find((p) => p.id === agent.cli)
  const providerLabel = preset?.label ?? agent.cli
  const baseModel = agent.modelTier
    ? `${TIER_LABELS[agent.modelTier]} (${agent.model})`
    : agent.model
  const effort = normalizeReasoningEffort(agent.reasoningEffort)
  const effortText = effort ? ` · ${EFFORT_LABELS[effort]}` : ''
  const prefix = agent.followDefaultLlm ? 'Padrão global · ' : ''
  return `${prefix}${providerLabel} · ${baseModel}${effortText}`
})

const backupSummary = computed(() => {
  const agent = selected.value
  if (!agent) return 'Fallback global'
  if (agent.backupMode === 'ask') return 'Perguntar antes'
  if (agent.backupMode === 'auto') {
    const count = agent.fallback.length
    return count > 0 ? `Automático (${count} ${count === 1 ? 'modelo' : 'modelos'})` : 'Automático (0 modelos)'
  }
  if (agent.fallback.length > 0) {
    return `${agent.fallback.length} ${agent.fallback.length === 1 ? 'modelo' : 'modelos'}`
  }
  return 'Fallback global'
})

const chatColorInfo = computed(() => findChatColor(selected.value?.chatColor))

const notificationSummary = computed(() => {
  const mode = selected.value?.notificationMode
  if (mode === 'needs_me') return 'Só quando precisar de mim'
  if (mode === 'mute') return 'Silenciar'
  return 'Padrão do app (Todas)'
})

const transportSummary = computed(() => {
  const agent = selected.value
  if (!agent) return ''
  const preset = roster.presets.find((p) => p.id === agent.cli)
  const transport = preset?.toolTransport ?? (preset?.supportsMcp ? 'mcp-native' : 'none')
  if (transport === 'mcp-native') return 'MCP nativo'
  if (transport === 'vokia-cli') return 'Vokia CLI'
  return 'Só texto'
})

const permissionsSummary = computed(() => {
  const mode = executionMode.modeOf(projects.selected?.id)
  return EXECUTION_MODE_INFO[mode]?.label ?? 'Leitura'
})

const foldersSummary = computed(() => {
  const count = selected.value?.extraDirs?.length ?? 0
  if (count === 0) return 'Nenhuma adicionada'
  return count === 1 ? '1 pasta' : `${count} pastas`
})

const instructionsSummary = computed(() => {
  const len = selected.value?.extraInstructions.trim().length ?? 0
  return len > 0 ? `${len} chars` : 'Padrão'
})

function buildDraft(agent: AgentProfile, overridesDraft: Partial<AgentDraft> = {}): AgentDraft {
  return {
    name: agent.name,
    role: agent.role,
    avatar: agent.avatar,
    cli: agent.cli,
    model: agent.model,
    modelTier: agent.modelTier ?? null,
    reasoningEffort: agent.reasoningEffort ?? null,
    command: agent.command,
    followDefaultLlm: agent.followDefaultLlm,
    fallback: agent.fallback,
    title: agent.title ?? null,
    description: agent.description ?? null,
    chatColor: agent.chatColor ?? null,
    extraDirs: agent.extraDirs ?? [],
    notificationMode: agent.notificationMode ?? null,
    backupMode: agent.backupMode ?? null,
    ...overridesDraft,
  }
}

async function patchSelected(changes: Partial<AgentDraft>): Promise<void> {
  const agent = selected.value
  if (!agent) return
  saving.value = true
  failure.value = null
  try {
    await roster.update(agent.id, buildDraft(agent, changes))
    identitySaved.value = true
    setTimeout(() => {
      identitySaved.value = false
    }, 1500)
  } catch (error) {
    failure.value = String(error)
  } finally {
    saving.value = false
  }
}

function scheduleIdentitySave(): void {
  const agent = selected.value
  if (!agent || agent.builtin) return
  if (identityTimer) clearTimeout(identityTimer)
  identityTimer = setTimeout(() => {
    const nextName = draftName.value.trim()
    const nextTitle = draftTitle.value.trim() || null
    const nextDesc = draftDescription.value.trim() || null
    if (
      nextName === agent.name &&
      nextTitle === (agent.title ?? null) &&
      nextDesc === (agent.description ?? null)
    ) {
      return
    }
    if (!nextName) return
    void patchSelected({
      name: nextName,
      title: nextTitle,
      description: nextDesc,
    })
  }, 450)
}

async function handleModelSelect(payload: ModelSelectionPayload): Promise<void> {
  const agent = selected.value
  modelPickerOpen.value = false
  if (!agent) return
  saving.value = true
  failure.value = null
  try {
    if (agent.builtin) {
      await roster.updateEmber({
        cli: payload.cli,
        modelTier: null,
        model: payload.model,
        reasoningEffort: payload.reasoningEffort,
        fallback: agent.fallback,
      })
    } else {
      const nextPreset = roster.presets.find((p) => p.id === payload.cli)
      const nextCommand =
        payload.cli !== agent.cli && nextPreset?.defaultCommand
          ? nextPreset.defaultCommand
          : agent.command
      await roster.update(
        agent.id,
        buildDraft(agent, {
          cli: payload.cli,
          model: payload.model,
          modelTier: null,
          reasoningEffort: payload.reasoningEffort,
          followDefaultLlm: false,
          command: nextCommand,
        }),
      )
    }
  } catch (error) {
    failure.value = String(error)
  } finally {
    saving.value = false
  }
}

function openSubpanel(id: AgentSubpanelId, event: MouseEvent): void {
  subpanelTrigger.value = event.currentTarget instanceof HTMLElement ? event.currentTarget : null
  activeSubpanel.value = id
}

function closeSubpanel(): void {
  activeSubpanel.value = null
  void nextTick(() => {
    subpanelTrigger.value?.focus()
    subpanelTrigger.value = null
  })
}

function openUsageForAgent(): void {
  const agent = selected.value
  if (!agent) return
  void usageAnalytics.setAgentFilter(agent.id)
  navigation.goTo('estatisticas')
}

function openAgentMemories(): void {
  const agent = selected.value
  learning.setAgentFilter(agent ? agent.id : null)
  navigation.goTo('memoria')
}

function onSaved(profile: AgentProfile): void {
  selectedId.value = profile.id
}

async function removeSelected(): Promise<void> {
  const agent = selected.value
  if (!agent || removing.value) return
  removing.value = true
  failure.value = null
  try {
    await roster.remove(agent.id)
    activeSubpanel.value = null
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
      <div class="flex flex-wrap items-end gap-2">
        <UiField v-model="query" class="min-w-[200px] flex-1" :label="SEARCH_LABEL" />
        <div class="flex flex-wrap items-center gap-2">
          <UiButton variant="quiet" size="lg" @press="providers.openSetup()">{{ CONFIGURE_LLMS_LABEL }}</UiButton>
          <UiCreateButton @press="dialog = {}">{{ NEW_AGENT_LABEL }}</UiCreateButton>
        </div>
      </div>
      <p v-if="visible.length === 0" class="text-caption text-muted">{{ NO_AGENTS_TEXT }}</p>
      <button
        v-for="agent in visible"
        :key="agent.id"
        type="button"
        :class="[ROW_BASE_CLASSES, selected?.id === agent.id ? ROW_ACTIVE_CLASSES : ROW_INACTIVE_CLASSES]"
        :aria-pressed="selected?.id === agent.id"
        @click="selectedId = agent.id"
      >
        <UiSketchBox :class="FRAME_CLASSES" :seed="hashSeed(agent.id)">
          <AgentPortrait :who="agent.id" :size="PORTRAIT_SIZE" />
        </UiSketchBox>
        <span class="flex min-w-0 flex-1 flex-col">
          <span class="flex items-center gap-1.5">
            <b :class="NAME_CLASSES">{{ agent.name }}</b>
            <span
              v-if="findChatColor(agent.chatColor)"
              class="size-2.5 shrink-0 rounded-full border border-project-line"
              :class="findChatColor(agent.chatColor)?.swatchClass"
              aria-hidden="true"
            />
            <TemporaryOverrideBadge
              v-if="overrides.byAgent(agent.id)"
              :override="overrides.byAgent(agent.id)!"
              :restoring="Boolean(overrides.restoring[agent.id])"
              @restore="overrides.restore(agent.id)"
            />
          </span>
          <span class="truncate text-caption text-muted">{{ agent.title || agent.role }}</span>
        </span>
      </button>
    </div>

    <aside v-if="selected" :class="FICHA_CLASSES">
        <!-- Cabeçalho de Identidade -->
        <div class="flex flex-col items-center gap-3 pb-1">
          <div class="relative">
            <UiSketchBox :class="FRAME_CLASSES" :seed="hashSeed(selected.id)" bold>
              <AgentPortrait :who="selected.id" :size="FICHA_PORTRAIT_SIZE" />
            </UiSketchBox>
            <button
              v-if="!selected.builtin"
              type="button"
              class="absolute -right-1.5 -bottom-1.5 grid size-6 cursor-pointer place-items-center rounded-full border border-line bg-surface text-tiny text-ink shadow-seg hover:bg-surface-2"
              :class="INK_FOCUS_CLASSES"
              aria-label="Editar avatar"
              title="Editar avatar"
              @click="openSubpanel('avatar', $event)"
            >
              &#9998;
            </button>
          </div>

          <div class="flex items-center gap-2">
            <span class="text-caption text-muted">{{ selected.role }}</span>
            <TemporaryOverrideBadge
              v-if="overrides.byAgent(selected.id)"
              :override="overrides.byAgent(selected.id)!"
              :restoring="Boolean(overrides.restoring[selected.id])"
              @restore="overrides.restore(selected.id)"
            />
            <span v-if="identitySaved" class="text-tiny text-ok">Salvo</span>
          </div>

          <div class="flex w-full flex-col gap-2">
            <label class="flex flex-col gap-1">
              <span class="text-small font-medium text-muted">Nome</span>
              <input
                v-model="draftName"
                type="text"
                :class="IDENTITY_INPUT_CLASSES"
                :disabled="selected.builtin"
                @input="scheduleIdentitySave()"
              />
            </label>
            <label class="flex flex-col gap-1">
              <span class="text-small font-medium text-muted">Título</span>
              <input
                v-model="draftTitle"
                type="text"
                maxlength="80"
                :placeholder="TITLE_PLACEHOLDER"
                :class="IDENTITY_INPUT_CLASSES"
                :disabled="selected.builtin"
                @input="scheduleIdentitySave()"
              />
            </label>
            <label class="flex flex-col gap-1">
              <span class="text-small font-medium text-muted">Descrição</span>
              <textarea
                v-model="draftDescription"
                rows="2"
                maxlength="500"
                :placeholder="DESCRIPTION_PLACEHOLDER"
                :class="IDENTITY_INPUT_CLASSES"
                :disabled="selected.builtin"
                @input="scheduleIdentitySave()"
              />
            </label>
          </div>
          <p v-if="selected.builtin" class="w-full text-small text-muted">{{ BUILTIN_TEXT }}</p>
        </div>

        <UiNotice v-if="failure !== null" :text="failure" @close="failure = null" />

        <!-- Cérebro -->
        <section class="flex flex-col gap-1.5">
          <h3 :class="SECTION_TITLE_CLASSES">{{ SECTION_BRAIN }}</h3>
          <div :class="SECTION_CARD_CLASSES">
            <AgentSettingsRow
              label="Modelo"
              :summary="modelSummary"
              chevron="down"
              @press="modelPickerOpen = true"
            />
            <AgentSettingsRow
              label="Modelos de reserva"
              :summary="backupSummary"
              @press="openSubpanel('backup', $event)"
            />
            <AgentSettingsRow
              label="Cor do chat"
              :summary="chatColorInfo?.label ?? 'Nenhuma'"
              :swatch-class="chatColorInfo?.swatchClass ?? null"
              @press="openSubpanel('chatColor', $event)"
            />
          </div>
        </section>

        <!-- Sabe -->
        <section class="flex flex-col gap-1.5">
          <h3 :class="SECTION_TITLE_CLASSES">{{ SECTION_KNOWS }}</h3>
          <div :class="SECTION_CARD_CLASSES">
            <AgentSettingsRow
              label="Instruções"
              :summary="instructionsSummary"
              @press="openSubpanel('instructions', $event)"
            />
            <AgentSettingsRow
              label="Memórias"
              :summary="`${learning.memories.length} salvas`"
              @press="openAgentMemories()"
            />
            <AgentSettingsRow
              label="Habilidades"
              summary="0 atribuídas"
              @press="openSubpanel('skills', $event)"
            />
            <AgentSettingsRow
              label="Arquivos"
              summary="0 B"
              @press="openSubpanel('files', $event)"
            />
            <AgentSettingsRow
              label="Artefatos"
              :summary="`${artifacts.documents.length} ligados`"
              @press="openSubpanel('artifacts', $event)"
            />
          </div>
        </section>

        <!-- Faz -->
        <section class="flex flex-col gap-1.5">
          <h3 :class="SECTION_TITLE_CLASSES">{{ SECTION_DOES }}</h3>
          <div :class="SECTION_CARD_CLASSES">
            <AgentSettingsRow
              label="Rotinas"
              summary="0 configuradas"
              @press="openSubpanel('routines', $event)"
            />
            <AgentSettingsRow
              label="Uso"
              summary="Ver estatísticas"
              @press="openUsageForAgent()"
            />
            <AgentSettingsRow
              label="Apps / MCP"
              :summary="transportSummary"
              @press="openSubpanel('mcp', $event)"
            />
            <AgentSettingsRow
              label="Exportar / Compartilhar agente"
              summary="JSON"
              @press="openSubpanel('export', $event)"
            />
          </div>
        </section>

        <!-- Regras -->
        <section class="flex flex-col gap-1.5">
          <h3 :class="SECTION_TITLE_CLASSES">{{ SECTION_RULES }}</h3>
          <div :class="SECTION_CARD_CLASSES">
            <AgentSettingsRow
              label="Permissões"
              :summary="permissionsSummary"
              @press="openSubpanel('permissions', $event)"
            />
            <AgentSettingsRow
              label="Pastas"
              :summary="foldersSummary"
              :chevron="(selected.extraDirs?.length ?? 0) === 0 ? 'plus' : 'right'"
              @press="openSubpanel('folders', $event)"
            />
            <AgentSettingsRow
              label="Avançado"
              @press="openSubpanel('advanced', $event)"
            />
          </div>
        </section>

        <!-- Me notificar -->
        <section class="flex flex-col gap-1.5">
          <h3 :class="SECTION_TITLE_CLASSES">{{ SECTION_NOTIFY }}</h3>
          <div :class="SECTION_CARD_CLASSES">
            <AgentSettingsRow
              label="Modo"
              :summary="notificationSummary"
              @press="openSubpanel('notifications', $event)"
            />
          </div>
          <p class="px-1 text-small text-muted">{{ NOTIFICATION_NOTE }}</p>
        </section>
    </aside>

    <AgentSettingsSubpanel
      v-if="selected && activeSubpanel !== null"
      :agent="selected"
      :subpanel="activeSubpanel"
      :saving="saving"
      :removing="removing"
      :failure="failure"
      @back="closeSubpanel()"
      @patch="patchSelected($event)"
      @remove="removeSelected()"
      @clear-failure="failure = null"
    />

    <ProviderModelPickerModal
      v-if="selected"
      :open="modelPickerOpen"
      title="Selecionar modelo do agente"
      :selected-provider-id="selected.cli"
      :selected-model="selected.model"
      :selected-model-tier="selected.modelTier ?? null"
      :selected-effort="normalizeReasoningEffort(selected.reasoningEffort)"
      :allowed-provider-ids="
        selected.builtin
          ? roster.presets
              .filter((p) => (p.toolTransport ?? (p.supportsMcp ? 'mcp-native' : 'none')) !== 'none')
              .map((p) => p.id)
          : null
      "
      :show-backup-shortcut="true"
      @close="modelPickerOpen = false"
      @select="handleModelSelect"
      @open-backup="modelPickerOpen = false; activeSubpanel = 'backup'"
    />

    <NewAgentDialog
      v-if="dialog"
      :agent="dialog.agent"
      @saved="onSaved"
      @close="dialog = null"
    />
  </div>
</template>
