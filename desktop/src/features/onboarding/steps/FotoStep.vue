<script setup lang="ts">
import UserAvatarPicker from '../../profile/UserAvatarPicker.vue'
import { useProfileDraft } from '../../profile/useProfileDraft'
import StepHeading from './StepHeading.vue'

const draft = useProfileDraft()
</script>

<template>
  <div class="flex w-full flex-col items-center gap-8">
    <StepHeading title="Escolha sua foto" text="Escolha um avatar para continuar." />
    <UserAvatarPicker />
    <p v-if="draft.error" role="alert" class="text-small text-danger">{{ draft.error }}</p>
  </div>
</template>
