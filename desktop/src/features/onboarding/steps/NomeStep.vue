<script setup lang="ts">
import { onMounted } from 'vue'
import VokiaMascot from '../../brand/VokiaMascot.vue'
import UserNameField from '../../profile/UserNameField.vue'
import { useProfileDraft } from '../../profile/useProfileDraft'
import StepHeading from './StepHeading.vue'

const MASCOT_SIZE = 72

const draft = useProfileDraft()
onMounted(draft.begin)
</script>

<template>
  <div class="flex w-full flex-col items-center gap-8">
    <StepHeading title="Como devemos te chamar?" text="De 2 a 20 caracteres." />
    <div class="flex w-full max-w-120 flex-col items-center gap-4">
      <VokiaMascot look="feliz" :size="MASCOT_SIZE" />
      <UserNameField />
      <p v-if="draft.error" role="alert" class="text-small text-danger">{{ draft.error }}</p>
    </div>
  </div>
</template>
