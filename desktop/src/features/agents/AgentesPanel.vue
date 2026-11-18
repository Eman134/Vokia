<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiField from '../../ui/UiField.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import type { AgentProfile } from '../team/teamApi'
import { normalizeName, useRoster } from '../team/useRoster'
import NewAgentDialog from './NewAgentDialog.vue'
import {
  BUILTIN_TEXT,
  EDIT_LABEL,
  NEW_AGENT_LABEL,
  NO_AGENTS_TEXT,
  SEARCH_LABEL,
} from './agentesCopy'
import { CLI_LABEL, MODEL_LABEL, ROLE_LABEL } from './agentFormCopy'

const COLUMNS_CLASSES = 'flex flex-wrap items-start gap-5'
const LIST_CLASSES = 'flex min-w-0 basis-[320px] flex-1 flex-col gap-3 rounded-[28px] bg-surface p-5'
const FICHA_CLASSES = 'flex min-w-0 basis-[360px] flex-col gap-4 rounded-[28px] border border-line p-6'
const ROW_BASE_CLASSES = 'flex items-center gap-3 rounded-control border px-3 py-2 text-left'
const ROW_ACTIVE_CLASSES = 'border-accent bg-accent-soft'
const ROW_INACTIVE_CLASSES = 'border-line'
const PORTRAIT_SIZE = 32
const FICHA_PORTRAIT_SIZE = 56
const MUTED_TEXT_CLASSES = 'text-caption text-muted'

const roster = useRoster()
const query = ref('')
const selectedId = ref<string | null>(null)
const dialog = ref<{ agent?: AgentProfile } | null>(null)

const visible = computed(() => {
  const key = normalizeName(query.value.trim())
  return roster.agents.filter(
    (agent) => key === '' || normalizeName(agent.name).includes(key) || normalizeName(agent.role).includes(key),
  )
})
const selected = computed(
  () => roster.agents.find((agent) => agent.id === selectedId.value) ?? roster.agents[0] ?? null,
)

function onSaved(profile: AgentProfile): void {
  selectedId.value = profile.id
}
</script>

<template>
  <div :class="COLUMNS_CLASSES">
    <div :class="LIST_CLASSES">
      <div class="flex items-end gap-2">
        <UiField v-model="query" class="min-w-0 flex-1" :label="SEARCH_LABEL" />
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
        <span class="flex min-w-0 flex-col">
          <b class="truncate text-ink">{{ agent.name }}</b>
          <span class="truncate text-caption text-muted">{{ agent.role }}</span>
        </span>
      </button>
    </div>

    <aside v-if="selected" :class="FICHA_CLASSES">
      <div class="flex items-center gap-3">
        <AgentPortrait :who="selected.id" :size="FICHA_PORTRAIT_SIZE" />
        <div class="flex min-w-0 flex-col">
          <b class="truncate text-control text-ink">{{ selected.name }}</b>
          <span class="truncate text-caption text-muted">{{ selected.role }}</span>
        </div>
      </div>
      <dl class="flex flex-col gap-2 text-caption">
        <div class="flex gap-2"><dt class="w-24 text-muted">{{ ROLE_LABEL }}</dt><dd class="text-ink">{{ selected.role }}</dd></div>
        <div class="flex gap-2"><dt class="w-24 text-muted">{{ CLI_LABEL }}</dt><dd class="text-ink">{{ selected.cli }}</dd></div>
        <div class="flex gap-2"><dt class="w-24 text-muted">{{ MODEL_LABEL }}</dt><dd class="text-ink">{{ selected.model }}</dd></div>
      </dl>
      <p v-if="selected.builtin" :class="MUTED_TEXT_CLASSES">{{ BUILTIN_TEXT }}</p>
      <div class="flex justify-end">
        <UiButton
          variant="quiet"
          size="lg"
          :disabled="selected.builtin"
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
