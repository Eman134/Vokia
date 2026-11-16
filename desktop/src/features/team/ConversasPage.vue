<script setup lang="ts">
import { computed } from 'vue'
import { useConsole } from '../console/useConsole'
import { useProjects } from '../projects/useProjects'
import AgentList from './AgentList.vue'
import Conversation from './Conversation.vue'
import QueuePanel from './QueuePanel.vue'

const PAGE_CLASSES = 'flex min-w-0 flex-1 flex-col gap-5 overflow-hidden px-4 pb-[150px] pt-7 xl:pl-2 xl:pr-8'
const TITLE_CLASSES = 'mt-1 text-[32px] font-bold tracking-[-0.03em] text-ink'
const PANELS_CLASSES = 'flex min-h-0 flex-1 flex-col gap-3.5 overflow-y-auto xl:flex-row xl:overflow-hidden'
const SUBTITLE = 'Cada agente tem a sua conversa: o que ele disse, o que rodou no terminal e o que vem na fila dele.'

const projectsStore = useProjects()
const consoleStore = useConsole()
const projectName = computed(() => projectsStore.selected?.name ?? '')
</script>

<template>
  <main :class="PAGE_CLASSES">
    <header>
      <span v-if="projectName" class="text-caption text-muted">{{ projectName }}</span>
      <h1 :class="TITLE_CLASSES">Conversas</h1>
      <p class="mt-1 max-w-[64ch] text-body text-muted">{{ SUBTITLE }}</p>
    </header>
    <div :class="PANELS_CLASSES">
      <AgentList />
      <Conversation />
      <QueuePanel v-if="consoleStore.activeAgent === null" />
    </div>
  </main>
</template>
