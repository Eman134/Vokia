<script setup lang="ts">
import { useId } from 'vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'

defineProps<{ agentName: string }>()
const model = defineModel<string>({ required: true })
const emit = defineEmits<{ submit: [] }>()

const NOTE_ROWS = 3
const NOTE_PLACEHOLDER = 'Ex.: reaproveite o fluxo do cartão; não mexa no schema de pedidos.'
const NOTE_HINT = 'Opcional. Fica salva no item, vai junto com o pedido e tem prioridade sobre o resto.'
const CONTROL_CLASSES = `w-full min-w-0 resize-y rounded-md border border-line bg-surface px-3 py-2 text-control text-ink placeholder:text-faint ${INK_FOCUS_CLASSES}`

const fieldId = useId()
const hintId = useId()
</script>

<template>
  <div class="flex flex-col gap-1">
    <label :for="fieldId" class="font-hand text-annotation text-muted">e diga a {{ agentName }}:</label>
    <textarea
      :id="fieldId"
      v-model="model"
      :rows="NOTE_ROWS"
      :placeholder="NOTE_PLACEHOLDER"
      :aria-describedby="hintId"
      :class="CONTROL_CLASSES"
      data-testid="plan-note"
      @keydown.ctrl.enter.prevent="emit('submit')"
      @keydown.meta.enter.prevent="emit('submit')"
    />
    <small :id="hintId" class="text-meta text-muted">{{ NOTE_HINT }}</small>
  </div>
</template>
