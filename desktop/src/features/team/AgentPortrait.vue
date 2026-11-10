<script setup lang="ts">
import { computed } from 'vue'
import { renderAvatarArt, resolveAvatarProfile } from '../identities/avatarProfiles'
import { avatarOf } from './agentAvatars'
import { DEFAULT_PORTRAIT_SIZE } from './portraitArt'

const props = withDefaults(defineProps<{ who: string; size?: number }>(), {
  size: DEFAULT_PORTRAIT_SIZE,
})

const art = computed(() => renderAvatarArt(resolveAvatarProfile(avatarOf(props.who))))
const label = computed(() => `Avatar de ${props.who.charAt(0).toUpperCase()}${props.who.slice(1)}`)
</script>

<template>
  <svg :width="size" :height="size" viewBox="0 0 64 64" role="img" :aria-label="label">
    <g v-html="art" />
  </svg>
</template>
