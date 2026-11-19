<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import { useProjects } from '../projects/useProjects'
import AgentList from './AgentList.vue'
import Conversation from './Conversation.vue'
import QueuePanel from './QueuePanel.vue'
import { useConversationTab } from './conversationTabs'
import { useQueueItems } from './useQueueItems'

const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col gap-5 overflow-hidden ${PAGE_INSET_CLASSES}`
const PANELS_CLASSES = 'flex min-h-0 flex-1 flex-col gap-3.5 overflow-y-auto md:flex-row md:overflow-hidden'
// Abaixo de lg (1024 px) não cabe a coluna de fila: ela vira aba da conversa.
const WIDE_MEDIA_QUERY = '(min-width: 64rem)'

const projectsStore = useProjects()
const conversationTab = useConversationTab()
// Sem item na fila a seção (e a aba dela no modo estreito) não aparece.
const queueItems = useQueueItems()
const hasQueue = computed(() => queueItems.value.length > 0)
const projectName = computed(() => projectsStore.selected?.name ?? '')
const wideMedia = window.matchMedia(WIDE_MEDIA_QUERY)
const wide = ref(wideMedia.matches)

function onWideChange(event: MediaQueryListEvent): void {
  wide.value = event.matches
}

onMounted(() => wideMedia.addEventListener('change', onWideChange))
onUnmounted(() => {
  wideMedia.removeEventListener('change', onWideChange)
  conversationTab.reset()
})
</script>

<template>
  <main :class="PAGE_CLASSES">
    <UiPageHeader title="Conversas" :project-name="projectName" />
    <div :class="PANELS_CLASSES">
      <AgentList />
      <Conversation :fila-em-aba="!wide && hasQueue">
        <template #fila>
          <QueuePanel />
        </template>
      </Conversation>
      <QueuePanel v-if="wide && hasQueue" />
    </div>
  </main>
</template>
