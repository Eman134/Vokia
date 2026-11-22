<script setup lang="ts">
import { computed } from 'vue'
import { initialOf } from './useUserProfile'
import { useProfileDraft } from './useProfileDraft'
import { USER_ICONS, type UserIcon } from './userIcons'
import UserAvatar from './UserAvatar.vue'

import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
const OPTION_SIZE = 48
const ENTRY_STAGGER_MS = 40
const PHOTO_LABEL = 'Sua foto'

const draft = useProfileDraft()
const initial = computed(() => initialOf(draft.name))

function isSelected(icon: UserIcon): boolean {
  return draft.avatar?.icone === icon.id
}

function select(icon: UserIcon): void {
  draft.avatar = { icone: icon.id }
}
</script>

<template>
  <div role="group" :aria-label="PHOTO_LABEL" class="flex w-full flex-wrap justify-center gap-4">
    <button
      v-for="(icon, index) in USER_ICONS"
      :key="icon.id"
      type="button"
      :aria-pressed="isSelected(icon)"
      :aria-label="icon.label"
      :title="icon.label"
      class="icon-option grid size-15 cursor-pointer place-items-center rounded-full p-1.5"
      :class="[INK_FOCUS_CLASSES, isSelected(icon) ? 'bg-accent-soft outline-2 outline-solid outline-offset-2 outline-accent' : 'bg-transparent hover:bg-hover']"
      :style="{ animationDelay: `${index * ENTRY_STAGGER_MS}ms` }"
      @click="select(icon)"
    >
      <UserAvatar :size="OPTION_SIZE" :avatar="{ icone: icon.id }" :initial="initial" />
    </button>
  </div>
</template>

<style scoped>
.icon-option {
  animation: icon-enter 360ms ease-out both;
  transition: transform 160ms ease-out;
}
.icon-option:hover {
  transform: translateY(-3px) scale(1.06);
}

@keyframes icon-enter {
  from {
    opacity: 0;
    transform: translateY(8px) scale(0.9);
  }
}

@media (prefers-reduced-motion: reduce) {
  .icon-option {
    animation: none;
    transition: none;
  }
  .icon-option:hover {
    transform: none;
  }
}
</style>
