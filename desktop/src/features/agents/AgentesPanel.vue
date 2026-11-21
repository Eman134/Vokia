<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiField from '../../ui/UiField.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { useProviders } from '../providers/useProviders'
import { useTemporaryOverrides } from '../providers/useTemporaryOverrides'
import AgentPortrait from '../team/AgentPortrait.vue'
import type { AgentProfile, ModelTier } from '../team/teamApi'
import { normalizeName, useRoster } from '../team/useRoster'
import AgentInstructions from './AgentInstructions.vue'
import NewAgentDialog from './NewAgentDialog.vue'
import TemporaryOverrideBadge from './TemporaryOverrideBadge.vue'
import {
  BUILTIN_TEXT,
  CONFIGURE_LLMS_LABEL,
  DEFAULT_PROVIDER_LABEL,
  DELETE_AGENT_CONFIRM_LABEL,
  DELETE_AGENT_MESSAGE,
  DELETE_LABEL,
  EDIT_LABEL,
  EFFORT_LEVEL_LABEL,
  FALLBACK_PROVIDER_LABEL,
  NEW_AGENT_LABEL,
  NO_AGENTS_TEXT,
  SEARCH_LABEL,
} from './agentesCopy'
import { ROLE_LABEL } from './agentFormCopy'

const COLUMNS_CLASSES = 'flex flex-wrap items-start gap-5'
const LIST_CLASSES = 'flex min-w-0 basis-[320px] flex-1 flex-col gap-3 rounded-[28px] bg-surface p-5'
const FICHA_CLASSES = 'flex min-w-0 basis-[360px] flex-col gap-4 rounded-[28px] border border-line p-6'
const ROW_BASE_CLASSES = 'flex cursor-pointer items-center gap-3 rounded-control border px-3 py-2 text-left'
const ROW_ACTIVE_CLASSES = 'border-accent bg-accent-soft'
const ROW_INACTIVE_CLASSES = 'border-line hover:bg-hover'
const PORTRAIT_SIZE = 32
const FICHA_PORTRAIT_SIZE = 56
const MUTED_TEXT_CLASSES = 'text-caption text-muted'

const TIER_LABELS: Record<ModelTier, string> = {
  basic: 'Básico',
  intermediate: 'Intermediário',
  advanced: 'Avançado',
}

const roster = useRoster()
const providers = useProviders()
const overrides = useTemporaryOverrides()
const query = ref('')
const selectedId = ref<string | null>(null)
const dialog = ref<{ agent?: AgentProfile } | null>(null)
const failure = ref<string | null>(null)
const removing = ref(false)

const visible = computed(() => {
  const key = normalizeName(query.value.trim())
  return roster.agents.filter(
    (agent) => key === '' || normalizeName(agent.name).includes(key) || normalizeName(agent.role).includes(key),
  )
})
const selected = computed(
  () => roster.agents.find((agent) => agent.id === selectedId.value) ?? roster.agents[0] ?? null,
)

function effortSummary(agent: AgentProfile): string {
  if (agent.modelTier) {
    return `${TIER_LABELS[agent.modelTier]} (${agent.model})`
  }
  return agent.model
}

function fallbackSummary(agent: AgentProfile): string {
  if (agent.fallback.length === 0) return 'Fallback global'
  return agent.fallback.map((target) => target.cli).join(', ')
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
    selectedId.value = null
  } catch (error) {
    // O backend recusa com o motivo (posição em time ou camada); o agente continua no elenco.
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
        <UiField v-model="query" class="min-w-0 flex-1" :label="SEARCH_LABEL" />
        <UiButton variant="quiet" size="lg" @press="providers.openSetup()">{{ CONFIGURE_LLMS_LABEL }}</UiButton>
        <UiButton variant="primary" size="lg" @press="dialog = {}">{{ NEW_AGENT_LABEL }}</UiButton>
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
        <AgentPortrait :who="agent.id" :size="PORTRAIT_SIZE" />
        <span class="flex min-w-0 flex-1 flex-col">
          <span class="flex items-center gap-1.5">
            <b class="truncate text-ink">{{ agent.name }}</b>
            <TemporaryOverrideBadge
              v-if="overrides.byAgent(agent.id)"
              :override="overrides.byAgent(agent.id)!"
              :restoring="Boolean(overrides.restoring[agent.id])"
              @restore="overrides.restore(agent.id)"
            />
          </span>
          <span class="truncate text-caption text-muted">{{ agent.role }}</span>
        </span>
      </button>
    </div>

    <aside v-if="selected" :class="FICHA_CLASSES">
      <div class="flex items-center gap-3">
        <AgentPortrait :who="selected.id" :size="FICHA_PORTRAIT_SIZE" />
        <div class="flex min-w-0 flex-1 flex-col">
          <div class="flex items-center gap-2">
            <b class="truncate text-control text-ink">{{ selected.name }}</b>
            <TemporaryOverrideBadge
              v-if="overrides.byAgent(selected.id)"
              :override="overrides.byAgent(selected.id)!"
              :restoring="Boolean(overrides.restoring[selected.id])"
              @restore="overrides.restore(selected.id)"
            />
          </div>
          <span class="truncate text-caption text-muted">{{ selected.role }}</span>
        </div>
      </div>
      <dl class="flex flex-col gap-2 text-caption">
        <div class="flex gap-2"><dt class="w-36 shrink-0 text-muted">{{ ROLE_LABEL }}</dt><dd class="text-ink">{{ selected.role }}</dd></div>
        <div class="flex gap-2">
          <dt class="w-36 shrink-0 text-muted">{{ DEFAULT_PROVIDER_LABEL }}</dt>
          <dd class="flex flex-wrap items-center gap-1.5 text-ink">
            <span>{{ selected.followDefaultLlm ? 'Padrão global' : selected.cli }}</span>
            <span
              v-if="!selected.followDefaultLlm"
              class="inline-flex h-5 items-center rounded-chip bg-surface-2 px-2 text-small font-medium text-muted"
            >
              {{
                (roster.presets.find((p) => p.id === selected.cli)?.toolTransport ??
                  (roster.presets.find((p) => p.id === selected.cli)?.supportsMcp ? 'mcp-native' : 'none')) === 'mcp-native'
                  ? 'MCP nativo'
                  : roster.presets.find((p) => p.id === selected.cli)?.toolTransport === 'vokia-cli'
                    ? 'Vokia CLI'
                    : 'Só texto'
              }}
            </span>
          </dd>
        </div>
        <div class="flex gap-2">
          <dt class="w-36 shrink-0 text-muted">{{ EFFORT_LEVEL_LABEL }}</dt>
          <dd class="text-ink">{{ effortSummary(selected) }}</dd>
        </div>
        <div class="flex gap-2">
          <dt class="w-36 shrink-0 text-muted">{{ FALLBACK_PROVIDER_LABEL }}</dt>
          <dd class="text-ink">{{ fallbackSummary(selected) }}</dd>
        </div>
      </dl>
      <AgentInstructions :key="selected.id" :agent="selected" />
      <p v-if="selected.builtin" :class="MUTED_TEXT_CLASSES">{{ BUILTIN_TEXT }}</p>
      <UiNotice v-if="failure !== null" :text="failure" @close="failure = null" />
      <div class="flex flex-wrap items-center justify-end gap-2">
        <UiConfirm
          v-if="!selected.builtin"
          :label="DELETE_LABEL"
          :message="DELETE_AGENT_MESSAGE"
          :confirm-label="DELETE_AGENT_CONFIRM_LABEL"
          :busy="removing"
          @confirm="removeSelected()"
        />
        <UiButton
          variant="quiet"
          size="lg"
          @press="dialog = { agent: selected }"
        >
          {{ EDIT_LABEL }}
        </UiButton>
      </div>
    </aside>

    <NewAgentDialog
      v-if="dialog"
      :agent="dialog.agent"
      @saved="onSaved"
      @close="dialog = null"
    />
  </div>
</template>
