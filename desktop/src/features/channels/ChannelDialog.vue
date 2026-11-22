<script setup lang="ts">
import { onMounted, ref } from 'vue'
import ChannelForm from './ChannelForm.vue'
import type { Channel } from './channelsApi'
import { trapFocus } from '../backlog/dialogFocus'
import UiCloseButton from '../../ui/UiCloseButton.vue'

const TITLE_ID = 'canal-titulo'
const NEW_TITLE = 'Novo canal'
const EDIT_TITLE = 'Editar canal'
const NEW_SUBTITLE = 'Crie um espaço focado para discussões com agentes específicos.'
const EDIT_SUBTITLE = 'Altere o nome ou quem participa deste canal.'
const OVERLAY_CLASSES = 'fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4'
const DIALOG_CLASSES =
  'flex max-h-full w-120 max-w-full flex-col gap-5 overflow-y-auto rounded-md border border-line bg-surface p-6 shadow-paper'
const ENTER_FROM_CLASS = 'opacity-0'
const ENTER_ACTIVE_CLASS = 'transition-opacity duration-200 ease-out motion-reduce:transition-none'
const BADGE_CLASSES =
  'grid size-11 shrink-0 place-items-center rounded-md bg-linear-to-br from-accent-soft to-surface-2 text-section font-bold text-accent ring-1 ring-line'

defineProps<{ channel?: Channel }>()
const emit = defineEmits<{ close: [] }>()

const dialog = ref<HTMLElement>()

onMounted(() => dialog.value?.querySelector<HTMLElement>('input')?.focus())
</script>

<template>
  <Transition appear :enter-from-class="ENTER_FROM_CLASS" :enter-active-class="ENTER_ACTIVE_CLASS">
    <div :class="OVERLAY_CLASSES" @click.self="emit('close')">
      <section
        ref="dialog"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="TITLE_ID"
        tabindex="-1"
        :class="DIALOG_CLASSES"
        @keydown.esc.stop="emit('close')"
        @keydown.tab="trapFocus($event, dialog)"
      >
        <header class="flex items-start gap-3">
          <span :class="BADGE_CLASSES" aria-hidden="true">#</span>
          <div class="flex min-w-0 flex-1 flex-col gap-0.5">
            <h2 :id="TITLE_ID" class="text-control font-bold text-ink">{{ channel ? EDIT_TITLE : NEW_TITLE }}</h2>
            <p class="text-caption text-muted">{{ channel ? EDIT_SUBTITLE : NEW_SUBTITLE }}</p>
          </div>
          <UiCloseButton @press="emit('close')" />
        </header>
        <ChannelForm :channel="channel" @done="emit('close')" @cancel="emit('close')" />
      </section>
    </div>
  </Transition>
</template>
