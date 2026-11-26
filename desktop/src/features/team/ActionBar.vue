<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch, type ComponentPublicInstance } from 'vue'
import { motion } from 'motion-v'
import type { Agent } from './events'
import type { ComposeSize } from './pageActions'
import { mentionSpans } from './composerMentions'
import { useConnection } from './useConnection'
import { useFeed } from './useFeed'
import { useFlow } from './useFlow'
import { useRoster } from './useRoster'
import { useTeamStatus } from './useTeamStatus'
import { useRequests } from './useRequests'
import { useDecisions } from '../decisions/useDecisions'
import { useAgentControl } from '../panel/useAgentControl'
import { useProjects } from '../projects/useProjects'
import { useUsageLimit } from '../usage/useUsageLimit'
import AgentPicker from '../agents/AgentPicker.vue'
import AgentPortrait from './AgentPortrait.vue'
import ActionMenu from './ActionMenu.vue'
import PermissionInline from './PermissionInline.vue'
import PinnedProject from './PinnedProject.vue'
import PausedAgents from './PausedAgents.vue'
import UsageLimitNotice from './UsageLimitNotice.vue'
import FlowSteps from './FlowSteps.vue'
import Balloon from '../balloon/Balloon.vue'
import { useDelegationNotices } from '../balloon/useDelegationNotices'
import { useErrorNotices } from '../balloon/useErrorNotices'
import PanelShell from '../panel/PanelShell.vue'
import { BAR_COLLAPSE_TOTAL_MS, BAR_FADE_CLASSES, BAR_SPRING, BAR_WRAP_CLASSES, PILL_CLASSES } from './bar/barClasses'
import ComposerField from './bar/ComposerField.vue'
import ExecutionModeSelector from './bar/ExecutionModeSelector.vue'
import DecisionsButton from './bar/DecisionsButton.vue'
import OfflineNotice from './bar/OfflineNotice.vue'
import PageActionsButton from './bar/PageActionsButton.vue'
import QueueButton from './bar/QueueButton.vue'
import StatusRing from './bar/StatusRing.vue'
import ActiveLlmSwitcher from '../providers/ActiveLlmSwitcher.vue'
import { useComposerSubmit } from './bar/useComposerSubmit'
import { hasComposerContent, showsInlinePermission } from './bar/inlinePermission'
import { usePageActions } from './bar/usePageActions'
import { listenFileDrops } from '../attachments/attachmentsApi'

const COMPACT_WIDTH_CLASS = 'w-fit max-w-[min(32rem,100%)]'
const COLLAPSE_MS = BAR_COLLAPSE_TOTAL_MS
const EXPANDED_WIDTH_CLASS = 'w-full max-w-full'
// Em janelas estreitas os controles ao redor do campo tomam a largura toda; menos espaço entre eles devolve o mínimo ao campo.
const NARROW_GAP_CLASS = 'max-sm:gap-1.5'
// Abaixo de sm a coluna do trilho deixaria a pílula estreita demais para os controles: ela passa a ocupar a janela com 16 px de margem.
const NARROW_WRAP_CLASS = 'max-sm:left-4 max-sm:right-4'
const CONTENT_WIDTH_CLASS = 'w-fit max-w-full'

const emit = defineEmits<{
  compose: [size: ComposeSize]
}>()

const connection = useConnection()
const feed = useFeed()
const flow = useFlow()
const roster = useRoster()
const projectsStore = useProjects()
const pillRef = ref<ComponentPublicInstance | null>(null)
const pillElement = computed(() => pillRef.value?.$el ?? null)
const { draft, confirmed, deliveryMode, attachments, submit } = useComposerSubmit()
const composer = ref<InstanceType<typeof ComposerField> | null>(null)
const hovered = ref(false)
const collapsing = ref(false)
let collapseTimer: ReturnType<typeof setTimeout> | undefined
const focusWithin = ref(false)
const actionMenuOpen = ref(false)
const queueOpen = ref(false)
const menuOpen = ref(false)
const llmSwitcherOpen = ref(false)
const modeOpen = ref(false)
const agentPickerOpen = ref(false)
const { railPage, runAction } = usePageActions(actionMenuOpen, (size) => emit('compose', size))

const hasDraft = computed(() => hasComposerContent(draft.value, attachments.attachments.value.length, confirmed.value.size))
const interacting = computed(
  () =>
    hasDraft.value ||
    hovered.value ||
    focusWithin.value ||
    menuOpen.value ||
    actionMenuOpen.value ||
    queueOpen.value ||
    llmSwitcherOpen.value ||
    modeOpen.value ||
    agentPickerOpen.value,
)
const expanded = computed(() => flow.active || interacting.value)
const inlinePermission = computed(() =>
  showsInlinePermission({
    flowActive: flow.active,
    expanded: expanded.value,
    pendingCount: feed.permissions.length,
    hasContent: hasDraft.value,
  }),
)
const canAsk = computed(() => flow.flow !== 'onboarding')
const statusStore = useTeamStatus()
const requests = useRequests()
const decisions = useDecisions()
const control = useAgentControl()
const usageLimit = useUsageLimit()
useErrorNotices()
useDelegationNotices()
const selectedRecipient = computed<Agent | null>(() => mentionSpans(draft.value, confirmed.value)[0]?.agent ?? null)
const queuedCounts = computed<Record<string, number>>(() => {
  const counts: Record<string, number> = {}
  for (const entry of requests.queue) {
    counts[entry.to] = (counts[entry.to] ?? 0) + 1
  }
  return counts
})
const recipientTooltip = computed(() =>
  selectedRecipient.value
    ? `Destinatário: ${roster.nameOf(selectedRecipient.value)} (trocar agente)`
    : 'Escolher agente destinatário',
)

function queueCountLabel(count: number): string {
  return `${count} na fila`
}

function onSelectRecipient(agentId: Agent | null): void {
  if (!agentId) return
  void nextTick(() => composer.value?.selectRecipient(agentId))
}
// Com layoutDependency o Motion só remede quando esta chave muda: hover, fluxo e conteúdo dos filhos entram aqui.
const layoutKey = computed(() =>
  [
    interacting.value,
    flow.active,
    flow.flow,
    flow.step,
    statusStore.statusText,
    decisions.pendingCount,
    requests.queue.length,
    control.pausedList.length,
    usageLimit.visible,
    feed.permissions.length,
    feed.activeAgents.length,
    attachments.attachments.value.length,
    attachments.saving.value,
  ].join('|'),
)
const pillWidthClass = computed(() => {
  if (canAsk.value && interacting.value) {
    return EXPANDED_WIDTH_CLASS
  }
  return flow.active ? CONTENT_WIDTH_CLASS : COMPACT_WIDTH_CLASS
})

// Sem isto o rótulo é cortado com "…" nos quadros em que a pílula já está mais estreita que o texto.
watch(interacting, (active) => {
  clearTimeout(collapseTimer)
  collapsing.value = !active
  if (!active) {
    collapseTimer = setTimeout(() => (collapsing.value = false), COLLAPSE_MS)
  }
})

watch(menuOpen, (open) => {
  if (open) {
    actionMenuOpen.value = false
    queueOpen.value = false
    llmSwitcherOpen.value = false
    modeOpen.value = false
    agentPickerOpen.value = false
  }
})

watch(actionMenuOpen, (open) => {
  if (open) {
    queueOpen.value = false
    llmSwitcherOpen.value = false
    modeOpen.value = false
    agentPickerOpen.value = false
  }
})

watch(queueOpen, (open) => {
  if (open) {
    actionMenuOpen.value = false
    llmSwitcherOpen.value = false
    modeOpen.value = false
    agentPickerOpen.value = false
  }
})

watch(llmSwitcherOpen, (open) => {
  if (open) {
    actionMenuOpen.value = false
    queueOpen.value = false
    modeOpen.value = false
    agentPickerOpen.value = false
  }
})

watch(modeOpen, (open) => {
  if (open) {
    actionMenuOpen.value = false
    queueOpen.value = false
    llmSwitcherOpen.value = false
    agentPickerOpen.value = false
  }
})

watch(agentPickerOpen, (open) => {
  if (open) {
    actionMenuOpen.value = false
    queueOpen.value = false
    llmSwitcherOpen.value = false
    modeOpen.value = false
  }
})

// O pointerleave se perde quando a pílula muda sob o cursor parado; sem reconciliar, hovered fica true e a barra não recolhe.
watch(hovered, (isHovered, _previous, onCleanup) => {
  if (!isHovered) {
    return
  }
  document.addEventListener('pointermove', reconcileHover)
  onCleanup(() => document.removeEventListener('pointermove', reconcileHover))
})

// O ouvinte vive o tempo do componente: a barra é desmontada offline e remontada, e um ouvinte esquecido duplicaria anexos.
let stopFileDrops: (() => void) | undefined
let unmounted = false

onMounted(async () => {
  const stop = await listenFileDrops((paths) => {
    if (canAsk.value && connection.online) {
      void attachments.addPaths(paths)
    }
  })
  if (unmounted) {
    stop()
  } else {
    stopFileDrops = stop
  }
})

onUnmounted(() => {
  unmounted = true
  stopFileDrops?.()
  clearTimeout(collapseTimer)
})

function onFocusOut(event: FocusEvent): void {
  if (!(event.currentTarget as HTMLElement).contains(event.relatedTarget as Node | null)) {
    focusWithin.value = false
  }
}

function reconcileHover(): void {
  if (!pillElement.value?.matches(':hover')) {
    hovered.value = false
  }
}

function onPointerLeave(): void {
  hovered.value = false
  if (!composer.value?.isFocused()) {
    actionMenuOpen.value = false
  }
}
</script>

<template>
  <div v-if="connection.online" :class="BAR_FADE_CLASSES" aria-hidden="true" data-testid="action-bar-fade" />
  <div v-if="connection.online" :class="[BAR_WRAP_CLASSES, NARROW_WRAP_CLASS]">
    <Balloon />
    <PanelShell />
    <motion.form
      ref="pillRef"
      layout
      :layout-dependency="layoutKey"
      :transition="BAR_SPRING"
      :class="[PILL_CLASSES, NARROW_GAP_CLASS, pillWidthClass]"
      @pointerenter="hovered = true"
      @pointerleave="onPointerLeave"
      @focusin="focusWithin = true"
      @focusout="onFocusOut"
      @submit.prevent="submit"
    >
      <StatusRing :expanded="expanded" :collapsing="collapsing" :can-ask="canAsk" />
      <PermissionInline v-if="inlinePermission" />
      <PausedAgents />
      <UsageLimitNotice />
      <FlowSteps v-if="flow.active" />
      <div class="flex shrink-0 items-center gap-0.5 empty:hidden">
        <PageActionsButton v-model:open="actionMenuOpen" :visible="expanded && !inlinePermission && !flow.active" />
        <ExecutionModeSelector v-if="canAsk && !flow.active && !inlinePermission" v-model:open="modeOpen" />
        <PinnedProject v-if="canAsk && projectsStore.projects.length > 1" />
        <ActiveLlmSwitcher v-if="canAsk && expanded && !inlinePermission && !flow.active" v-model:open="llmSwitcherOpen" />
        <AgentPicker
          v-if="canAsk && expanded && !inlinePermission && !flow.active && roster.agents.length > 0"
          v-model:open="agentPickerOpen"
          :model-value="selectedRecipient"
          :counts="queuedCounts"
          :count-label="queueCountLabel"
          :disabled="connection.checking"
          busy-first
          placement="top"
          floating="absolute"
          trigger-variant="ghost"
          :trigger-class="selectedRecipient ? 'h-7 max-w-28 px-1.5' : 'w-7 justify-center px-0'"
          label="Escolher agente destinatário"
          :title="recipientTooltip"
          test-id="action-bar-agent-picker"
          @select="onSelectRecipient"
        >
          <template #trigger="{ agent }">
            <AgentPortrait v-if="agent" :who="agent.id" :size="18" class="shrink-0" />
            <svg
              v-else
              class="size-4 shrink-0"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <circle cx="12" cy="12" r="4" />
              <path d="M16 8v5a3 3 0 0 0 6 0v-1a10 10 0 1 0-4 8" />
            </svg>
            <span v-if="agent" class="truncate text-small font-medium text-ink">{{ agent.name }}</span>
          </template>
        </AgentPicker>
      </div>
      <ComposerField
        ref="composer"
        v-model:draft="draft"
        v-model:confirmed="confirmed"
        v-model:delivery-mode="deliveryMode"
        :can-ask="canAsk"
        :interacting="interacting"
        :inline-permission="inlinePermission"
        :rail-page="railPage"
        :pill="pillElement"
        :attachments="attachments"
        @menu-open="menuOpen = $event"
      />
      <DecisionsButton :expanded="expanded" />
      <QueueButton v-model:open="queueOpen" />
      <ActionMenu
        v-if="actionMenuOpen && railPage !== null"
        :page="railPage"
        @run="runAction"
        @close="actionMenuOpen = false"
      />
    </motion.form>
  </div>
  <OfflineNotice />
</template>
