<script setup lang="ts">
import { computed } from 'vue'
import VokiaMascot from '../../brand/VokiaMascot.vue'
import { useUserProfile } from '../../profile/useUserProfile'
import StepHeading from './StepHeading.vue'

const MASCOT_SIZE = 92

const profile = useUserProfile()
const title = computed(() => (profile.name ? `Tudo pronto, ${profile.name}` : 'Tudo pronto'))
</script>

<template>
  <div class="flex w-full flex-col gap-7">
    <StepHeading :title="title" text="Seu perfil fica em Configurações, onde dá para mudar nome e foto quando quiser.">
      <template #mascot>
        <VokiaMascot look="animado" :size="MASCOT_SIZE" />
      </template>
    </StepHeading>
  </div>
</template>
