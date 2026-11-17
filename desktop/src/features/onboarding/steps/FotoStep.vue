<script setup lang="ts">
import { onMounted } from 'vue'
import UserAvatarPicker from '../../profile/UserAvatarPicker.vue'
import { useProfileDraft } from '../../profile/useProfileDraft'
import StepHeading from './StepHeading.vue'

const draft = useProfileDraft()
// O rascunho vem da etapa do nome; só recomeça se esta etapa abriu sem ela.
onMounted(() => {
  if (draft.name === '') draft.begin()
})
</script>

<template>
  <div class="flex w-full flex-col items-center gap-8">
    <StepHeading title="Escolha sua foto" text="É opcional. Sem foto, aparece a inicial do seu nome." />
    <UserAvatarPicker />
    <p v-if="draft.error" role="alert" class="text-small text-danger">{{ draft.error }}</p>
  </div>
</template>
