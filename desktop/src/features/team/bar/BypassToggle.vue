<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from 'vue'
import { trapFocus } from '../../backlog/dialogFocus'
import { useBypass } from '../../permissions/useBypass'
import { useProjects } from '../../projects/useProjects'
import UiButton from '../../../ui/UiButton.vue'
import UiCloseButton from '../../../ui/UiCloseButton.vue'

const props = defineProps<{ expanded: boolean }>()

const WARNING_TITLE = 'Ligar o modo bypass?'
const WARNING_TEXT =
  'Com o bypass ligado, os agentes deste projeto passam a ter os pedidos de permissão aprovados automaticamente, sem perguntar. Nada pede a sua confirmação, nem apagar nem push forçado fora do projeto. Use só em projetos em que você confia nos agentes; o modo desliga ao fechar o app.'
const ENABLE_LABEL = 'Ligar modo bypass'
const DISABLE_LABEL = 'Desligar modo bypass (aprovando permissões automaticamente)'
const OVERLAY_CLASSES = 'fixed inset-0 z-60 flex items-center justify-center bg-paper/70 p-4'
const FRAME_CLASSES = 'relative border border-line w-100 max-w-full rounded-md bg-surface shadow-paper'
const DIALOG_CLASSES = 'flex max-h-[calc(100dvh-2rem)] flex-col gap-4 overflow-y-auto p-5'
const ACTIVE_CLASSES = 'gap-1.5 bg-warn-soft text-warn-text'

const bypass = useBypass()
const projects = useProjects()
const projectId = computed(() => projects.selected?.id ?? null)
const active = computed(() => bypass.isOn(projectId.value))
const warning = ref(false)
const dialog = ref<HTMLElement>()
const cancelButton = ref<{ $el: HTMLElement } | null>(null)
const titleId = useId()
let origin: HTMLElement | null = null

watch(
  projectId,
  (id) => {
    if (id !== null) void bypass.load(id)
  },
  { immediate: true },
)

async function openWarning(): Promise<void> {
  origin = document.activeElement instanceof HTMLElement ? document.activeElement : null
  warning.value = true
  await nextTick()
  cancelButton.value?.$el.focus()
}

function closeWarning(): void {
  warning.value = false
  origin?.focus()
  origin = null
}

function turnOn(): void {
  if (projectId.value === null) return
  void bypass.set(projectId.value, true)
}

function acceptWarning(): void {
  bypass.acknowledgeWarning()
  closeWarning()
  turnOn()
}

function toggle(): void {
  if (projectId.value === null) return
  if (active.value) void bypass.set(projectId.value, false)
  else if (bypass.warningAcknowledged) turnOn()
  else void openWarning()
}
</script>

<template>
  <UiButton
    v-if="active || (props.expanded && projectId !== null)"
    variant="quiet"
    size="sm"
    :class="active && ACTIVE_CLASSES"
    :aria-label="active ? DISABLE_LABEL : ENABLE_LABEL"
    :aria-pressed="active"
    :title="active ? DISABLE_LABEL : ENABLE_LABEL"
    data-testid="bypass-toggle"
    @press="toggle"
  >
    <svg class="size-3.5 shrink-0" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M8 2l5 2v4c0 3-2.2 5-5 6-2.8-1-5-3-5-6V4z" />
    </svg>
    <span v-if="active" class="max-sm:hidden" data-testid="bypass-indicator">Bypass</span>
  </UiButton>
  <Teleport to="body">
    <div v-if="warning" :class="OVERLAY_CLASSES" @click.self="closeWarning">
      <div :class="FRAME_CLASSES">
        <UiCloseButton class="absolute right-3 top-3 z-10" @press="closeWarning" />
        <section
          ref="dialog"
          role="dialog"
          aria-modal="true"
          :aria-labelledby="titleId"
          tabindex="-1"
          :class="DIALOG_CLASSES"
          data-testid="bypass-warning"
          @keydown.esc.stop="closeWarning"
          @keydown.tab="trapFocus($event, dialog)"
        >
          <h3 :id="titleId" class="text-control font-bold text-ink">{{ WARNING_TITLE }}</h3>
          <p class="text-caption text-muted">{{ WARNING_TEXT }}</p>
          <div class="flex justify-end gap-2">
            <UiButton ref="cancelButton" variant="quiet" size="lg" @press="closeWarning">Cancelar</UiButton>
            <UiButton variant="danger" size="lg" data-testid="bypass-accept" @press="acceptWarning">Ligar bypass</UiButton>
          </div>
        </section>
      </div>
    </div>
  </Teleport>
</template>
