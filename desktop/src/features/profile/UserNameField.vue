<script setup lang="ts">
import { computed } from 'vue'
import { USER_NAME_MAX_CHARS } from './useUserProfile'
import { useProfileDraft } from './useProfileDraft'

const NAME_LABEL = 'Seu nome'
const NAME_INPUT_CLASSES =
  'w-full min-w-0 border-0 border-b border-line bg-transparent pb-2 text-center text-page font-semibold text-ink placeholder:text-faint focus:border-accent focus:outline-none'

const draft = useProfileDraft()
const counter = computed(() => `${[...draft.name.trim()].length}/${USER_NAME_MAX_CHARS}`)
</script>

<template>
  <div class="flex w-full flex-col items-center gap-1.5">
    <input
      v-model="draft.name"
      type="text"
      :aria-label="NAME_LABEL"
      :placeholder="NAME_LABEL"
      :aria-invalid="draft.name !== '' && !draft.nameValid"
      :class="NAME_INPUT_CLASSES"
    />
    <span class="text-small text-faint">{{ counter }}</span>
  </div>
</template>
