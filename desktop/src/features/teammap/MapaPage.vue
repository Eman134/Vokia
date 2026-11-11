<script setup lang="ts">
import { ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import NewAgentDialog from '../agents/NewAgentDialog.vue'
import type { AgentProfile } from '../team/teamApi'
import TeamMatrix from './TeamMatrix.vue'
import {
  ADD_AGENT_LABEL,
  ADD_LAYER_LABEL,
  ADD_TEAM_LABEL,
  MAP_SUBTITLE,
  MAP_TITLE,
  TAB_ORCHESTRATION,
  TAB_TEAMS,
} from './teamMapLayers'

const PAGE_CLASSES = 'flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto pt-7 pr-8 pb-[150px] pl-2'
const TAB_LIST_CLASSES = 'flex gap-0.5 rounded-control-lg bg-surface-2 p-0.75'
const TAB_BASE_CLASSES = 'rounded-[9px] px-3 py-1.5 text-caption disabled:opacity-40'
const TAB_ACTIVE_CLASSES = 'bg-surface font-semibold text-ink shadow-seg'
const TAB_INACTIVE_CLASSES = 'text-muted'

const creating = ref(false)
const highlightedId = ref<string | null>(null)

function onCreated(agent: AgentProfile): void {
  highlightedId.value = agent.id
}
</script>

<template>
  <section :class="PAGE_CLASSES">
    <header class="flex flex-wrap items-end gap-4">
      <div>
        <h1 class="mt-1 text-[32px] font-bold tracking-[-0.03em] text-ink">{{ MAP_TITLE }}</h1>
        <p class="mt-1 max-w-[64ch] text-body text-muted">{{ MAP_SUBTITLE }}</p>
      </div>
      <div class="ml-auto flex flex-wrap items-center gap-2">
        <div role="tablist" :class="TAB_LIST_CLASSES">
          <button type="button" role="tab" aria-selected="true" :class="[TAB_BASE_CLASSES, TAB_ACTIVE_CLASSES]">
            {{ TAB_TEAMS }}
          </button>
          <button
            type="button"
            role="tab"
            aria-selected="false"
            aria-disabled="true"
            disabled
            :class="[TAB_BASE_CLASSES, TAB_INACTIVE_CLASSES]"
          >
            {{ TAB_ORCHESTRATION }}
          </button>
        </div>
        <UiButton variant="quiet" size="lg" :disabled="true" aria-disabled="true">{{ ADD_TEAM_LABEL }}</UiButton>
        <UiButton variant="primary" size="lg" :disabled="true" aria-disabled="true">{{ ADD_LAYER_LABEL }}</UiButton>
        <UiButton variant="primary" size="lg" @press="creating = true">{{ ADD_AGENT_LABEL }}</UiButton>
      </div>
    </header>
    <TeamMatrix :highlighted-id="highlightedId" @add-agent="creating = true" />
    <NewAgentDialog v-if="creating" @created="onCreated" @close="creating = false" />
  </section>
</template>
