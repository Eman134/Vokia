<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useProjects } from '../projects/useProjects'
import AgentList from './AgentList.vue'
import Conversation from './Conversation.vue'
import QueuePanel from './QueuePanel.vue'
import { useConversationTab } from './conversationTabs'
import { useQueueItems } from './useQueueItems'

const PAGE_CLASSES = 'flex min-w-0 flex-1 flex-col gap-5 overflow-hidden px-4 pb-[150px] pt-7'
const TITLE_CLASSES = 'mt-1 text-[32px] font-bold tracking-[-0.03em] text-ink'
const PANELS_CLASSES = 'flex min-h-0 flex-1 flex-col gap-3.5 overflow-y-auto md:flex-row md:overflow-hidden'
// Abaixo de lg (1024 px) não cabe a coluna de fila: ela vira aba da conversa.
const WIDE_MEDIA_QUERY = '(min-width: 64rem)'
const SUBTITLE = 'Cada agente tem a sua conversa: o que ele disse e o que rodou no terminal.'
const SUBTITLE_WITH_QUEUE = 'Cada agente tem a sua conversa: o que ele disse, o que rodou no terminal e o que vem na fila dele.'

const projectsStore = useProjects()
const conversationTab = useConversationTab()
// Sem item na fila a seção (e a aba dela no modo estreito) não aparece.
const queueItems = useQueueItems()
const hasQueue = computed(() => queueItems.value.length > 0)
const subtitle = computed(() => (hasQueue.value ? SUBTITLE_WITH_QUEUE : SUBTITLE))
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
    <header>
      <span v-if="projectName" class="text-caption text-muted">{{ projectName }}</span>
      <h1 :class="TITLE_CLASSES">Conversas</h1>
      <p class="mt-1 max-w-[64ch] text-body text-muted">{{ subtitle }}</p>
    </header>
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
