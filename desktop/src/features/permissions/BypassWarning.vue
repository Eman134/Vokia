<script setup lang="ts">
import { nextTick, ref, useId, watch } from 'vue'
import { trapFocus } from '../backlog/dialogFocus'
import UiButton from '../../ui/UiButton.vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'

const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ accept: [] }>()

const WARNING_TITLE = 'Ligar o bypass total?'
const WARNING_TEXT =
  'Com o bypass total, os agentes deste projeto passam a ter os pedidos de permissão aprovados automaticamente, sem perguntar. Nada pede a sua confirmação, nem apagar nem push forçado fora do projeto. Use só em projetos em que você confia nos agentes; o modo volta a manual ao fechar o app.'
const OVERLAY_CLASSES = 'fixed inset-0 z-60 flex items-center justify-center bg-paper/70 p-4'
const FRAME_CLASSES = 'relative border border-line w-100 max-w-full rounded-md bg-surface shadow-paper'
const DIALOG_CLASSES = 'flex max-h-[calc(100dvh-2rem)] flex-col gap-4 overflow-y-auto p-5'

const dialog = ref<HTMLElement>()
const cancelButton = ref<{ $el: HTMLElement } | null>(null)
const titleId = useId()
let origin: HTMLElement | null = null

watch(open, async (isOpen) => {
  if (!isOpen) {
    origin?.focus()
    origin = null
    return
  }
  origin = document.activeElement instanceof HTMLElement ? document.activeElement : null
  await nextTick()
  cancelButton.value?.$el.focus()
})

function close(): void {
  open.value = false
}

function accept(): void {
  close()
  emit('accept')
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" :class="OVERLAY_CLASSES" @click.self="close">
      <div :class="FRAME_CLASSES">
        <UiCloseButton class="absolute right-3 top-3 z-10" @press="close" />
        <section
          ref="dialog"
          role="dialog"
          aria-modal="true"
          :aria-labelledby="titleId"
          tabindex="-1"
          :class="DIALOG_CLASSES"
          data-testid="bypass-warning"
          @keydown.esc.stop="close"
          @keydown.tab="trapFocus($event, dialog)"
        >
          <h3 :id="titleId" class="text-control font-bold text-ink">{{ WARNING_TITLE }}</h3>
          <p class="text-caption text-muted">{{ WARNING_TEXT }}</p>
          <div class="flex justify-end gap-2">
            <UiButton ref="cancelButton" variant="quiet" size="lg" @press="close">Cancelar</UiButton>
            <UiButton variant="danger" size="lg" data-testid="bypass-accept" @press="accept">Ligar bypass</UiButton>
          </div>
        </section>
      </div>
    </div>
  </Teleport>
</template>
