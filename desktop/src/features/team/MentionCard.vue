<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import AgentPortrait from './AgentPortrait.vue'
import { useMentionCard } from './useMentionCard'
import { useRoster } from './useRoster'

const CARD_WIDTH = 224
const VIEWPORT_MARGIN = 8
const ANCHOR_GAP = 6
const PORTRAIT_SIZE = 36

const card = useMentionCard()
const roster = useRoster()
const root = ref<HTMLElement>()

const profile = computed(() => (card.agent === null ? undefined : roster.profileOf(card.agent)))
const position = computed(() => {
  const anchor = card.anchor
  if (anchor === null) return {}
  const left = Math.max(VIEWPORT_MARGIN, Math.min(anchor.left, window.innerWidth - CARD_WIDTH - VIEWPORT_MARGIN))
  // A barra de ação fica no rodapé: na metade de baixo da janela o cartão abre para cima.
  return anchor.top > window.innerHeight / 2
    ? { left: `${left}px`, bottom: `${window.innerHeight - anchor.top + ANCHOR_GAP}px` }
    : { left: `${left}px`, top: `${anchor.bottom + ANCHOR_GAP}px` }
})

function closeOnOutsidePress(event: PointerEvent): void {
  if (card.agent !== null && !root.value?.contains(event.target as Node)) card.close()
}

function closeOnEscape(event: KeyboardEvent): void {
  if (event.key === 'Escape') card.close()
}

onMounted(() => {
  document.addEventListener('pointerdown', closeOnOutsidePress)
  document.addEventListener('keydown', closeOnEscape)
})
onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', closeOnOutsidePress)
  document.removeEventListener('keydown', closeOnEscape)
})
</script>

<template>
  <div
    v-if="card.agent !== null && profile"
    ref="root"
    data-testid="mention-card"
    class="fixed z-50"
    :style="{ ...position, width: `${CARD_WIDTH}px` }"
    @mouseenter="card.cancelClose()"
    @mouseleave="card.scheduleClose()"
  >
    <button
      type="button"
      class="flex w-full items-center gap-3 rounded-card bg-surface p-3 text-left shadow-float focus:outline-2 focus:outline-solid focus:outline-accent"
      :title="`Mencionar ${profile.name}`"
      @click="card.insert(profile.id)"
    >
      <AgentPortrait :who="profile.id" :size="PORTRAIT_SIZE" />
      <span class="flex min-w-0 flex-col">
        <b class="truncate text-control text-ink">{{ profile.name }}</b>
        <span class="truncate text-caption text-muted">{{ profile.role }}</span>
      </span>
    </button>
  </div>
</template>
