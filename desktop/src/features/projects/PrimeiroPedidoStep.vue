<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { storeToRefs } from 'pinia'
import { BACK_LABEL, CREATE_WITHOUT_REQUEST_LABEL, LINKED_REPO_LABEL } from './projetosCopy'
import { useNovoProjeto } from './useNovoProjeto'

const wizard = useNovoProjeto()
const { busy, error, linkedRepo, originKind } = storeToRefs(wizard)
</script>

<template>
  <div class="flex flex-col gap-5.5">
    <p v-if="originKind === 'folder' && linkedRepo" data-testid="linked-repo" class="text-caption text-muted">{{ LINKED_REPO_LABEL }} · {{ linkedRepo }}</p>

    <div v-if="error?.field === 'general'" data-testid="create-error">
      <UiNotice :text="error.message" @close="error = null" />
    </div>

    <div class="flex items-center justify-end gap-2">
      <UiButton variant="quiet" size="lg" data-testid="step-back" @press="wizard.back">{{ BACK_LABEL }}</UiButton>
      <UiButton variant="primary" size="lg" :disabled="busy" data-testid="step-create" @press="wizard.create">
        {{ CREATE_WITHOUT_REQUEST_LABEL }}
      </UiButton>
    </div>
  </div>
</template>
