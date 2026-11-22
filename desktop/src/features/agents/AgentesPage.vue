<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiNotice from '../../ui/UiNotice.vue'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { useRoster } from '../team/useRoster'
import AgentesPanel from './AgentesPanel.vue'
import PapeisPanel from './PapeisPanel.vue'
import { PAGE_SUBTITLE, PAGE_TITLE, TAB_AGENTS, TAB_ROLES } from './agentesCopy'
import { useRoles } from './useRoles'

type AgentesTab = 'agentes' | 'papeis'

const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto ${PAGE_INSET_CLASSES}`
const TABS: readonly { label: string; value: AgentesTab }[] = [
  { label: TAB_AGENTS, value: 'agentes' },
  { label: TAB_ROLES, value: 'papeis' },
]

const roster = useRoster()
const roles = useRoles()
const tab = ref<AgentesTab>('agentes')
const loadError = ref<string | null>(null)

async function loadAll(): Promise<void> {
  try {
    await Promise.all([roster.load(), roles.load()])
  } catch (error) {
    loadError.value = String(error)
  }
}

onMounted(loadAll)
</script>

<template>
  <section :class="PAGE_CLASSES">
    <UiPageHeader :title="PAGE_TITLE" :subtitle="PAGE_SUBTITLE" />
    <div class="flex shrink-0 items-center">
      <UiSegmented v-model="tab" :options="TABS" :label="PAGE_TITLE" wrap />
    </div>
    <UiNotice v-if="loadError !== null" :text="loadError" @close="loadError = null" />
    <AgentesPanel v-if="tab === 'agentes'" />
    <PapeisPanel v-else />
  </section>
</template>
