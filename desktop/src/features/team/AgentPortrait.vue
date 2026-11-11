<script setup lang="ts">
import { computed } from 'vue'
import { renderAvatarArt, resolveAvatarProfile } from '../identities/avatarProfiles'
import { useAvatarProfiles } from '../identities/useAvatarProfiles'
import { useRoster } from './useRoster'
import { DEFAULT_PORTRAIT_SIZE } from './portraitArt'

const props = withDefaults(defineProps<{ who: string; size?: number }>(), {
  size: DEFAULT_PORTRAIT_SIZE,
})

const roster = useRoster()
const avatarProfiles = useAvatarProfiles()

const art = computed(() => renderAvatarArt(resolveAvatarProfile(roster.avatarOf(props.who), avatarProfiles.profiles)))
const label = computed(() => {
  const profile = roster.profileOf(props.who)
  return `Avatar de ${profile?.name ?? `${props.who.charAt(0).toUpperCase()}${props.who.slice(1)}`}`
})
</script>

<template>
  <svg :width="size" :height="size" viewBox="0 0 64 64" role="img" :aria-label="label">
    <g v-html="art" />
  </svg>
</template>
