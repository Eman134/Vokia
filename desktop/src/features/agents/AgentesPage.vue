<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiNotice from '../../ui/UiNotice.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { useRoster } from '../team/useRoster'
import AgentesPanel from './AgentesPanel.vue'
import PapeisPanel from './PapeisPanel.vue'
import { PAGE_SUBTITLE, PAGE_TITLE, TAB_AGENTS, TAB_ROLES } from './agentesCopy'
import { useRoles } from './useRoles'

type AgentesTab = 'agentes' | 'papeis'

const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto ${PAGE_INSET_CLASSES}`
const TITLE_CLASSES = 'text-[32px] font-bold tracking-[-0.03em] text-ink'
const SUBTITLE_CLASSES = 'max-w-[64ch] text-body text-muted'
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
    <header class="flex shrink-0 flex-col gap-1">
      <h1 :class="TITLE_CLASSES">{{ PAGE_TITLE }}</h1>
      <p :class="SUBTITLE_CLASSES">{{ PAGE_SUBTITLE }}</p>
    </header>
    <div class="flex shrink-0 items-center">
      <UiSegmented v-model="tab" :options="TABS" :label="PAGE_TITLE" wrap />
    </div>
    <UiNotice v-if="loadError !== null" :text="loadError" @close="loadError = null" />
    <AgentesPanel v-if="tab === 'agentes'" />
    <PapeisPanel v-else />
  </section>
</template>
