<script setup lang="ts">
import { computed } from 'vue'
import { initialOf } from './useUserProfile'
import { useProfileDraft } from './useProfileDraft'
import { USER_ICONS, type UserIcon } from './userIcons'
import UserAvatar from './UserAvatar.vue'

const OPTION_SIZE = 48
const ENTRY_STAGGER_MS = 40
const PHOTO_LABEL = 'Sua foto'
const PHOTO_HINT = 'Sem foto, aparece a inicial do nome.'

const draft = useProfileDraft()
const initial = computed(() => initialOf(draft.name))

function isSelected(icon: UserIcon): boolean {
  return draft.avatar?.icone === icon.id
}

function toggle(icon: UserIcon): void {
  draft.avatar = isSelected(icon) ? null : { icone: icon.id }
}
</script>

<template>
  <div role="group" :aria-label="PHOTO_LABEL" class="flex flex-col items-center gap-5">
    <div class="grid grid-cols-4 gap-4 sm:grid-cols-8">
      <button
        v-for="(icon, index) in USER_ICONS"
        :key="icon.id"
        type="button"
        :aria-pressed="isSelected(icon)"
        :aria-label="icon.label"
        :title="icon.label"
        class="icon-option grid place-items-center rounded-full p-1.5 focus:outline-2 focus:outline-solid focus:outline-accent"
        :class="isSelected(icon) ? 'icon-selected bg-accent-soft' : 'bg-transparent'"
        :style="{ animationDelay: `${index * ENTRY_STAGGER_MS}ms` }"
        @click="toggle(icon)"
      >
        <UserAvatar :size="OPTION_SIZE" :avatar="{ icone: icon.id }" :initial="initial" />
      </button>
    </div>
    <span class="text-center text-small text-muted">{{ PHOTO_HINT }}</span>
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
.icon-selected {
  animation:
    icon-enter 360ms ease-out both,
    icon-bob 2.4s ease-in-out 400ms infinite;
}

@keyframes icon-enter {
  from {
    opacity: 0;
    transform: translateY(8px) scale(0.9);
  }
}
@keyframes icon-bob {
  50% {
    transform: translateY(-3px);
  }
}

@media (prefers-reduced-motion: reduce) {
  .icon-option,
  .icon-selected {
    animation: none;
    transition: none;
  }
  .icon-option:hover {
    transform: none;
  }
}
</style>
