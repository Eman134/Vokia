<script setup lang="ts">
import { computed } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import { HAND_DRAWN_RADIUS, STICKY_INK_VARS } from '../../ui/handDrawn'

const props = defineProps<{ agentName: string; interim?: string; pendingQuestion: boolean }>()
const emit = defineEmits<{ answer: [] }>()

// Só o fim do rascunho: o texto inteiro é a proposta que chega depois.
const DRAFT_TAIL_LINES = 8
const WAITING_TEXT = 'Ainda sem rascunho. O texto aparece aqui conforme chega.'
const QUESTION_TEXT = 'perguntou algo antes de continuar. A pergunta está no painel de Decisões.'
const STICKY_CLASSES = `flex flex-wrap items-center gap-2 bg-mark-yellow px-3 py-2 text-control text-ink shadow-paper ${HAND_DRAWN_RADIUS} ${STICKY_INK_VARS}`

const tail = computed(() => props.interim?.trim().split('\n').slice(-DRAFT_TAIL_LINES).join('\n') ?? '')
</script>

<template>
  <div class="flex flex-col gap-2.5" data-testid="plan-draft">
    <p class="flex items-center gap-2 text-control text-ink">
      <span><b>{{ agentName }}</b> está escrevendo o plano</span>
      <UiHandLine variant="scribble" draw class="h-3 w-12 text-graphite" />
    </p>
    <p v-if="tail" class="max-h-40 overflow-hidden border-l-[1.5px] border-line pl-3 font-mono text-meta whitespace-pre-wrap text-muted">{{ tail }}</p>
    <p v-else class="text-caption text-muted">{{ WAITING_TEXT }}</p>
    <div v-if="pendingQuestion" :class="STICKY_CLASSES">
      <span class="flex-1">{{ agentName }} {{ QUESTION_TEXT }}</span>
      <UiButton variant="primary" size="sm" @press="emit('answer')">Responder</UiButton>
    </div>
  </div>
</template>
