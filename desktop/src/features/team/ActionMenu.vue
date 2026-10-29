<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { PAGE_ACTIONS, type PageAction } from './pageActions'
import { PAGE_LABELS, type RailPage } from '../../shell/railNav'
import UiButton from '../../ui/UiButton.vue'

const props = defineProps<{
  page: RailPage
}>()

const emit = defineEmits<{
  run: [action: PageAction]
  close: []
}>()

const pageLabel = computed(() => PAGE_LABELS[props.page])
const actions = computed(() => PAGE_ACTIONS[props.page])

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && !event.defaultPrevented) {
    emit('close')
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div class="absolute bottom-full left-0 pb-2">
    <div class="w-60 rounded-popover border border-line bg-surface p-1.5 shadow-float">
      <p class="px-2 py-1.5 text-caption text-muted">Ações · {{ pageLabel }}</p>
      <ul role="menu" :aria-label="`Ações · ${pageLabel}`" class="flex flex-col gap-0.5">
        <li v-for="(action, index) in actions" :key="action.id" role="none">
          <UiButton variant="quiet" size="sm" role="menuitem" class="w-full" @press="emit('run', action)">
            <span class="flex w-full items-center justify-between">
              <span>{{ action.label }}</span>
              <span class="font-mono text-caption text-faint">⌥{{ index + 1 }}</span>
            </span>
          </UiButton>
        </li>
      </ul>
    </div>
  </div>
</template>
