<script setup lang="ts">
import { computed } from 'vue'
import { findUserIcon, type UserAvatarChoice } from './userIcons'
import { useUserProfile } from './useUserProfile'

const ICON_VIEW_BOX = 64
const ICON_RADIUS = ICON_VIEW_BOX / 2
const INITIAL_FONT_RATIO = 0.4

// avatar e initial só são passados para pré-visualizar; sem eles vale o perfil salvo.
const props = defineProps<{ size: number; avatar?: UserAvatarChoice | null; initial?: string }>()

const profile = useUserProfile()
const choice = computed(() => (props.avatar === undefined ? profile.avatar : props.avatar))
const icon = computed(() => (choice.value ? findUserIcon(choice.value.icone) : undefined))
const letter = computed(() => props.initial ?? profile.initial)
</script>

<template>
  <span
    class="relative inline-grid flex-none place-items-center rounded-full"
    :class="icon ? '' : 'border border-line bg-surface-2 font-semibold text-ink'"
    :style="{ width: `${size}px`, height: `${size}px`, fontSize: `${size * INITIAL_FONT_RATIO}px` }"
  >
    <svg v-if="icon" :width="size" :height="size" :viewBox="`0 0 ${ICON_VIEW_BOX} ${ICON_VIEW_BOX}`" aria-hidden="true" class="block">
      <circle :cx="ICON_RADIUS" :cy="ICON_RADIUS" :r="ICON_RADIUS" :fill="icon.background" />
      <g v-html="icon.glyph" />
    </svg>
    <template v-else>{{ letter }}</template>
  </span>
</template>
