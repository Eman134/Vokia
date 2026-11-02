<script lang="ts">
export const EMPTY_FEED_TITLE = 'Nenhuma mensagem ainda.'
export const EMPTY_FEED_TEXT = 'Peça a primeira na barra de ação.'
export const NO_PROJECT_TITLE = 'Nenhum projeto ainda.'
export const NO_PROJECT_TEXT = 'Crie um projeto para começar a conversar com o time.'
</script>

<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from 'vue'
import { AGENTS, useTeam } from './useTeam'
import MessageEntry from './MessageEntry.vue'
import PermissionPrompt from './PermissionPrompt.vue'
import QueuedEntry from './QueuedEntry.vue'
import VokiaMascot from '../brand/VokiaMascot.vue'
import { useProjects } from '../projects/useProjects'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiNotice from '../../ui/UiNotice.vue'

const AGENT_GROUP_TITLE = AGENTS.map((agent) => agent.name).join(' ↔ ')
const AT_END_TOLERANCE_PX = 4

const team = useTeam()
const projectsStore = useProjects()
const scroller = ref<HTMLElement | null>(null)
// Só o próprio scroll decide: arrastar a barra ou rolar com a roda sai do fim e volta ao chegar nele.
const followingEnd = ref(true)
const clearFailure = ref<string | null>(null)

function countLabel(count: number): string {
  return count === 1 ? '1 mensagem' : `${count} mensagens`
}

async function clearConversation(): Promise<void> {
  clearFailure.value = null
  try {
    await team.clearFeed()
  } catch (error) {
    clearFailure.value = `Não foi possível limpar a conversa: ${String(error)}`
  }
}

function onScroll(): void {
  const element = scroller.value
  if (element === null) return
  followingEnd.value = element.scrollHeight - element.scrollTop - element.clientHeight <= AT_END_TOLERANCE_PX
}

async function followEnd(): Promise<void> {
  await nextTick()
  const element = scroller.value
  if (followingEnd.value && element !== null) {
    element.scrollTop = element.scrollHeight
  }
}

watch([() => team.feed.length, () => team.queue.length, () => team.permissions.length, () => team.thinkingAgent], followEnd)
onMounted(followEnd)
</script>

<template>
  <section ref="scroller" class="flex-1 overflow-y-auto px-8 pb-40 pt-6" @scroll="onScroll">
    <header class="mb-6 flex items-center justify-between gap-4">
      <h1 class="text-title text-ink">Conversas</h1>
      <UiConfirm
        v-if="team.feed.length > 0"
        label="Limpar"
        message="Apaga todas as mensagens desta conversa. O agente recomeça sem esse histórico."
        confirm-label="Limpar conversa"
        @confirm="clearConversation"
      />
    </header>
    <UiNotice v-if="clearFailure" class="mb-6" :text="clearFailure" @close="clearFailure = null" />
    <UiEmpty v-if="projectsStore.loaded && projectsStore.selected === null" :title="NO_PROJECT_TITLE" :text="NO_PROJECT_TEXT">
      <template #mascot>
        <VokiaMascot look="tudo-em-dia" />
      </template>
    </UiEmpty>
    <UiEmpty v-else-if="team.feed.length === 0" :title="EMPTY_FEED_TITLE" :text="EMPTY_FEED_TEXT">
      <template #mascot>
        <VokiaMascot look="tudo-em-dia" />
      </template>
    </UiEmpty>
    <ol class="flex max-w-3xl flex-col gap-3">
      <li v-for="item in team.timeline" :key="item.start">
        <details v-if="item.type === 'group'" class="rounded-card border border-line bg-surface px-4 py-3">
          <summary class="flex cursor-pointer list-none items-baseline gap-2">
            <span class="text-caption font-semibold text-ink">{{ AGENT_GROUP_TITLE }}</span>
            <span class="text-caption text-muted">{{ countLabel(item.messages.length) }}</span>
          </summary>
          <ol class="mt-3 flex flex-col gap-3">
            <li v-for="(message, index) in item.messages" :key="index">
              <MessageEntry :entry="message" />
            </li>
          </ol>
        </details>
        <MessageEntry v-else :entry="item.entry" />
      </li>
    </ol>
    <ol v-if="team.permissions.length > 0" class="mt-3 flex max-w-3xl flex-col gap-3">
      <li v-for="permission in team.permissions" :key="permission.id">
        <PermissionPrompt :permission="permission" @decide="(allow) => team.decidePermission(permission.id, allow)" />
      </li>
    </ol>
    <ol v-if="team.queue.length > 0" class="mt-3 flex max-w-3xl flex-col gap-3">
      <li v-for="request in team.queue" :key="request.id">
        <QueuedEntry :request="request" />
      </li>
    </ol>
    <div
      role="status"
      :aria-label="team.thinkingAgent ? `${team.thinkingAgent.name} está pensando` : undefined"
      class="mt-3 flex max-w-3xl items-start gap-2.5"
    >
      <template v-if="team.thinkingAgent">
        <span
          class="grid size-7 shrink-0 place-items-center rounded-full border border-line bg-surface-2 text-small font-semibold text-ink"
        >
          {{ team.thinkingAgent.name.charAt(0) }}
        </span>
        <div class="min-w-0">
          <header class="flex items-baseline gap-2">
            <span class="text-caption font-semibold text-ink">{{ team.thinkingAgent.name }}</span>
            <span class="text-caption text-muted">pensando</span>
          </header>
          <div class="mt-1.5 flex items-center gap-1">
            <span class="size-1.5 rounded-full bg-muted motion-reduce:animate-none animate-dot-1" />
            <span class="size-1.5 rounded-full bg-muted motion-reduce:animate-none animate-dot-2" />
            <span class="size-1.5 rounded-full bg-muted motion-reduce:animate-none animate-dot-3" />
          </div>
        </div>
      </template>
    </div>
  </section>
</template>
