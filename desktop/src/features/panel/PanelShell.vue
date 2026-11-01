<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import DecisionPanel from '../decisions/DecisionPanel.vue'
import { PANEL_TITLE } from '../decisions/decisionCopy'
import { useDecisions } from '../decisions/useDecisions'

const TOGGLE_KEY_CODE = 'KeyJ'

const decisions = useDecisions()

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
    <DecisionPanel />
  </section>
</template>
