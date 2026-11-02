<script setup lang="ts">
import { ref } from 'vue'
import UiButton from './UiButton.vue'

defineProps<{
  label: string
  message: string
  confirmLabel: string
  busy?: boolean
}>()
const emit = defineEmits<{ confirm: [] }>()

const ROOT_CLASSES = 'flex min-w-0 flex-wrap items-center gap-2'
const MESSAGE_CLASSES = 'min-w-0 flex-1 text-caption text-muted'

const asking = ref(false)

function accept() {
  asking.value = false
  emit('confirm')
}
</script>

<template>
  <div :class="ROOT_CLASSES">
    <UiButton v-if="!asking" variant="danger" size="sm" :disabled="busy" data-testid="confirm-open" @press="asking = true">
      {{ label }}
    </UiButton>
    <template v-else>
      <p :class="MESSAGE_CLASSES">{{ message }}</p>
      <UiButton variant="quiet" size="sm" data-testid="confirm-cancel" @press="asking = false">Cancelar</UiButton>
      <UiButton variant="danger" size="sm" :disabled="busy" data-testid="confirm-accept" @press="accept">
        {{ confirmLabel }}
      </UiButton>
    </template>
  </div>
</template>
