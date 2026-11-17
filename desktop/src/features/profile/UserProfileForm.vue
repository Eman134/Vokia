<script setup lang="ts">
import { useProfileDraft } from './useProfileDraft'
import UserAvatarPicker from './UserAvatarPicker.vue'
import UserNameField from './UserNameField.vue'

const draft = useProfileDraft()
</script>

<template>
  <div class="flex w-full flex-col items-center gap-8">
    <UserNameField />
    <UserAvatarPicker />
    <p v-if="draft.error" role="alert" class="text-small text-danger">{{ draft.error }}</p>
  </div>
</template>
