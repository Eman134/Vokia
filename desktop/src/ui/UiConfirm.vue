<script setup lang="ts">
import { nextTick, ref, useId } from 'vue'
import { trapFocus } from '../features/backlog/dialogFocus'
import UiButton from './UiButton.vue'
import UiCloseButton from './UiCloseButton.vue'

defineProps<{
  label: string
  message: string
  confirmLabel: string
  busy?: boolean
}>()
const emit = defineEmits<{ confirm: [] }>()

const ROOT_CLASSES = 'flex min-w-0 flex-wrap items-center gap-2'
// Acima dos modais das telas (z-50): a confirmação pode abrir de dentro de um deles.
const OVERLAY_CLASSES = 'fixed inset-0 z-60 flex items-center justify-center bg-paper/70 p-4'
// Mesma estrutura dos demais diálogos (ModalPanel, remoção de fallback): moldura de papel, título em negrito, texto de apoio e ações à direita.
const FRAME_CLASSES = 'relative border border-line w-100 max-w-full rounded-md bg-surface shadow-paper'
// O backdrop tem p-4 (2rem no total): o diálogo rola por dentro em vez de passar da janela.
const DIALOG_CLASSES = 'flex max-h-[calc(100dvh-2rem)] flex-col gap-4 overflow-y-auto p-5'
const TITLE_CLASSES = 'text-control font-bold text-ink'
const MESSAGE_CLASSES = 'text-caption text-muted'
const ACTIONS_CLASSES = 'flex justify-end gap-2'

const asking = ref(false)
const dialog = ref<HTMLElement>()
const cancelButton = ref<{ $el: HTMLElement } | null>(null)
const titleId = useId()
let origin: HTMLElement | null = null

async function open(): Promise<void> {
  origin = document.activeElement instanceof HTMLElement ? document.activeElement : null
  asking.value = true
  await nextTick()
  // Cancelar é o foco inicial: Enter sem querer nunca executa a ação destrutiva.
  cancelButton.value?.$el.focus()
}

function close(): void {
  asking.value = false
  origin?.focus()
  origin = null
}

function accept(): void {
  close()
  emit('confirm')
}

// Os ouvintes de clique fora das telas rodam no document: parar a propagação evita que fechem o pai e desmontem a confirmação.
function onOverlayClick(event: MouseEvent): void {
  if (event.target === event.currentTarget) close()
}
</script>

<template>
  <div :class="ROOT_CLASSES">
    <UiButton variant="danger" size="sm" :disabled="busy" data-testid="confirm-open" @press="open">
      {{ label }}
    </UiButton>
    <Teleport to="body">
      <div
        v-if="asking"
        :class="OVERLAY_CLASSES"
        @click.stop="onOverlayClick"
        @pointerdown.stop
        @mousedown.stop
      >
        <div :class="FRAME_CLASSES">
      <UiCloseButton class="absolute right-3 top-3 z-10" @press="close" />
          <section
            ref="dialog"
            role="dialog"
            aria-modal="true"
            :aria-labelledby="titleId"
            tabindex="-1"
            :class="DIALOG_CLASSES"
            @keydown.esc.stop="close"
            @keydown.tab="trapFocus($event, dialog)"
          >
            <h3 :id="titleId" :class="TITLE_CLASSES">{{ confirmLabel }}</h3>
            <p :class="MESSAGE_CLASSES">{{ message }}</p>
            <div :class="ACTIONS_CLASSES">
              <UiButton ref="cancelButton" variant="quiet" size="lg" data-testid="confirm-cancel" @press="close">Cancelar</UiButton>
              <UiButton variant="danger" size="lg" :disabled="busy" data-testid="confirm-accept" @press="accept">
                {{ confirmLabel }}
              </UiButton>
            </div>
          </section>
        </div>
      </div>
    </Teleport>
  </div>
</template>
