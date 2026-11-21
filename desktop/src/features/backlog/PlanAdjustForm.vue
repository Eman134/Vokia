<script setup lang="ts">
import { ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'

const emit = defineEmits<{
  submit: [text: string]
}>()

const ADJUST_LABEL = 'O que ajustar'
const INPUT_CLASSES =
  'w-full rounded-control border border-line bg-surface px-3 py-2 text-control text-ink placeholder:text-faint'

const text = ref('')

function send(): void {
  const trimmed = text.value.trim()
  if (trimmed === '') return
  emit('submit', trimmed)
  text.value = ''
}
</script>

<template>
  <div class="flex flex-col gap-1.5">
    <textarea
      v-model="text"
      :aria-label="ADJUST_LABEL"
      :placeholder="ADJUST_LABEL"
      rows="3"
      :class="INPUT_CLASSES"
    />
    <UiButton
      class="self-start"
      variant="primary"
      size="sm"
      :disabled="text.trim() === ''"
      @press="send"
    >
      Enviar ajuste
    </UiButton>
  </div>
</template>
