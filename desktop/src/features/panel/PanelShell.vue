<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import NowPanel from './NowPanel.vue'
import { NOW_TAB_LABEL } from './nowCopy'
import { useTeam } from '../team/useTeam'
import DecisionPanel from '../decisions/DecisionPanel.vue'
import { PANEL_TITLE } from '../decisions/decisionCopy'
import { useDecisions } from '../decisions/useDecisions'

type PanelTab = 'decisoes' | 'agora'

interface TabEntry {
  id: PanelTab
  label: string
  count: number
}

const TOGGLE_KEY_CODE = 'KeyJ'

const decisions = useDecisions()
const team = useTeam()
const tab = ref<PanelTab>('decisoes')
const tabs = computed<TabEntry[]>(() => [
  { id: 'decisoes', label: PANEL_TITLE, count: decisions.pendingCount },
  { id: 'agora', label: NOW_TAB_LABEL, count: team.activeAgents.length },
])

function onKeydown(event: KeyboardEvent): void {
  if ((event.ctrlKey || event.metaKey) && event.code === TOGGLE_KEY_CODE) {
    event.preventDefault()
    decisions.toggle()
  } else if (event.key === 'Escape' && decisions.open && !event.defaultPrevented) {
    decisions.setOpen(false)
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <section
    v-if="decisions.open"
    :aria-label="PANEL_TITLE"
    data-testid="decision-panel"
    class="pointer-events-auto absolute bottom-full mb-3 w-96 max-w-full rounded-popover border border-line bg-surface p-3 shadow-float"
  >
    <div role="tablist" class="mb-2 flex gap-3">
      <button
        v-for="entry in tabs"
        :key="entry.id"
        type="button"
        role="tab"
        :aria-selected="tab === entry.id"
        class="text-label font-semibold focus:outline-2 focus:outline-solid focus:outline-accent"
        :class="tab === entry.id ? 'text-ink' : 'text-muted'"
        @click="tab = entry.id"
      >
        {{ entry.label }} <span data-testid="panel-tab-count">{{ entry.count }}</span>
      </button>
    </div>
    <DecisionPanel v-if="tab === 'decisoes'" />
    <NowPanel v-else />
  </section>
</template>
