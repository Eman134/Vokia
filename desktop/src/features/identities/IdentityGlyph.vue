<script setup lang="ts">
import { computed } from 'vue'
import { findIdentityConcept } from './identityFamilies'
import { STATE_MARKS, type IdentityState } from './identityState'

const props = defineProps<{
  family: string
  concept: string
  state: IdentityState
  size: number
}>()

const MARK_RATIO = 0.4
const MARK_FONT_RATIO = 0.6
const MARK_MIN_SIZE = 8
const MARK_MIN_FONT_SIZE = 7

const STATE_CLASSES: Record<IdentityState, string> = {
  executando: 'bg-ok',
  esperando: 'bg-warn',
  concluido: 'bg-ok',
  falha: 'bg-danger',
  pausado: 'bg-muted',
}

const art = computed(() => findIdentityConcept(props.family, props.concept).art)
const proportionalMarkSize = computed(() => props.size * MARK_RATIO)
const markSize = computed(() => Math.max(proportionalMarkSize.value, MARK_MIN_SIZE))
const markFontSize = computed(() => Math.max(proportionalMarkSize.value * MARK_FONT_RATIO, MARK_MIN_FONT_SIZE))
</script>

<template>
  <span
    role="img"
    :aria-label="state"
    class="relative inline-block flex-none"
    :style="{ width: `${size}px`, height: `${size}px` }"
  >
    <svg :width="size" :height="size" viewBox="0 0 64 64" overflow="visible" aria-hidden="true" class="block">
      <g v-html="art" />
    </svg>
    <span
      aria-hidden="true"
      class="absolute right-0 bottom-0 grid place-items-center rounded-full text-accent-ink"
      :class="STATE_CLASSES[state]"
      :style="{
        width: `${markSize}px`,
        height: `${markSize}px`,
        fontSize: `${markFontSize}px`,
        lineHeight: 1,
      }"
    >{{ STATE_MARKS[state] }}</span>
  </span>
</template>
