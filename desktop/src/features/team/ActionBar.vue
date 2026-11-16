<script setup lang="ts">
import { computed, onUnmounted, ref, watch, type ComponentPublicInstance } from 'vue'
import { motion } from 'motion-v'
import type { ComposeSize } from './pageActions'
import { useConnection } from './useConnection'
import { useFeed } from './useFeed'
import { useFlow } from './useFlow'
import { useTeamStatus } from './useTeamStatus'
import { useRequests } from './useRequests'
import { useDecisions } from '../decisions/useDecisions'
import { useAgentControl } from '../panel/useAgentControl'
import { useProjects } from '../projects/useProjects'
import ActionMenu from './ActionMenu.vue'
import PermissionInline from './PermissionInline.vue'
import PinnedProject from './PinnedProject.vue'
import PausedAgents from './PausedAgents.vue'
import FlowSteps from './FlowSteps.vue'
import Balloon from '../balloon/Balloon.vue'
import PanelShell from '../panel/PanelShell.vue'
import { BAR_COLLAPSE_TOTAL_MS, BAR_SPRING, BAR_WRAP_CLASSES, PILL_CLASSES } from './bar/barClasses'
import ComposerField from './bar/ComposerField.vue'
import DecisionsButton from './bar/DecisionsButton.vue'
import OfflineNotice from './bar/OfflineNotice.vue'
import PageActionsButton from './bar/PageActionsButton.vue'
import QueueButton from './bar/QueueButton.vue'
import StatusRing from './bar/StatusRing.vue'
import { useComposerSubmit } from './bar/useComposerSubmit'
import { usePageActions } from './bar/usePageActions'

const COMPACT_WIDTH_CLASS = 'w-fit max-w-[min(32rem,100%)]'
const COLLAPSE_MS = BAR_COLLAPSE_TOTAL_MS
const EXPANDED_WIDTH_CLASS = 'w-full max-w-full'
const CONTENT_WIDTH_CLASS = 'w-fit max-w-full'

const emit = defineEmits<{
  compose: [size: ComposeSize]
}>()

const connection = useConnection()
const feed = useFeed()
const flow = useFlow()
const projectsStore = useProjects()
const pillRef = ref<ComponentPublicInstance | null>(null)
const pillElement = computed(() => pillRef.value?.$el ?? null)
const { draft, confirmed, submit } = useComposerSubmit()
const composer = ref<InstanceType<typeof ComposerField> | null>(null)
const hovered = ref(false)
const collapsing = ref(false)
let collapseTimer: ReturnType<typeof setTimeout> | undefined
const focusWithin = ref(false)
const actionMenuOpen = ref(false)
const queueOpen = ref(false)
const menuOpen = ref(false)
const { railPage, runAction } = usePageActions(actionMenuOpen, (size) => emit('compose', size))

const hasDraft = computed(() => draft.value !== '')
const interacting = computed(
  () => hasDraft.value || hovered.value || focusWithin.value || menuOpen.value || actionMenuOpen.value || queueOpen.value,
)
const expanded = computed(() => flow.active || interacting.value)
const inlinePermission = computed(() => !flow.active && expanded.value && feed.permissions.length > 0)
const canAsk = computed(() => flow.flow !== 'onboarding')
const statusStore = useTeamStatus()
const requests = useRequests()
const decisions = useDecisions()
const control = useAgentControl()
// O `layout` do Motion só remede quando a própria barra re-renderiza; mudanças de conteúdo nos filhos precisam desta chave.
const layoutKey = computed(() =>
  [
    flow.flow,
    flow.step,
    statusStore.statusText,
    decisions.pendingCount,
    requests.queue.length,
    control.pausedList.length,
    feed.permissions.length,
    feed.activeAgents.length,
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
  }
})

watch(actionMenuOpen, (open) => {
  if (open) {
    queueOpen.value = false
  }
})

watch(queueOpen, (open) => {
  if (open) {
    actionMenuOpen.value = false
  }
})

onUnmounted(() => {
  clearTimeout(collapseTimer)
})

function onFocusOut(event: FocusEvent): void {
  if (!(event.currentTarget as HTMLElement).contains(event.relatedTarget as Node | null)) {
    focusWithin.value = false
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
  <div v-if="connection.online" :class="BAR_WRAP_CLASSES">
    <Balloon />
    <PanelShell />
    <motion.form
      ref="pillRef"
      layout
      :layout-dependency="layoutKey"
      :transition="BAR_SPRING"
      :class="[PILL_CLASSES, pillWidthClass]"
      @pointerenter="hovered = true"
      @pointerleave="onPointerLeave"
      @focusin="focusWithin = true"
      @focusout="onFocusOut"
      @submit.prevent="submit"
    >
      <StatusRing :expanded="expanded" :collapsing="collapsing" :can-ask="canAsk" />
      <PermissionInline v-if="inlinePermission" />
      <PausedAgents />
      <FlowSteps v-if="flow.active" />
      <PageActionsButton v-model:open="actionMenuOpen" :visible="expanded && !inlinePermission && !flow.active" />
      <ComposerField
        ref="composer"
        v-model:draft="draft"
        v-model:confirmed="confirmed"
        :can-ask="canAsk"
        :interacting="interacting"
        :inline-permission="inlinePermission"
        :rail-page="railPage"
        :pill="pillElement"
        @menu-open="menuOpen = $event"
      />
      <PinnedProject v-if="canAsk && projectsStore.projects.length > 1" />
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
