<script setup lang="ts">
import { onMounted, onUnmounted, watch } from 'vue'
import { useNavigation } from '../../../shell/useNavigation'
import { useLearningSuggestions } from '../../memory/useLearningSuggestions'
import UiBadge from '../../../ui/UiBadge.vue'
import UiButton from '../../../ui/UiButton.vue'

const emit = defineEmits<{ close: [] }>()

const learning = useLearningSuggestions()
const navigation = useNavigation()

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && !event.defaultPrevented) {
    emit('close')
  }
}

function openMemoryPage(): void {
  navigation.goTo('memoria')
  emit('close')
}

watch(
  () => learning.pendingCount,
  (count) => {
    if (count === 0) emit('close')
  },
  { immediate: true },
)

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div class="absolute bottom-full right-0 pb-2">
    <div
      class="w-88 max-w-[calc(100vw-2rem)] rounded-md border border-line bg-surface p-2 shadow-paper"
      role="dialog"
      aria-label="Sugestões de memória"
      data-testid="memory-suggestions-popover"
    >
      <div class="flex items-center justify-between px-1.5 pb-1.5">
        <p class="text-caption font-semibold text-ink">
          Sugestões de memória ({{ learning.pendingCount }})
        </p>
        <UiButton variant="quiet" size="sm" @press="openMemoryPage">
          Abrir Memória
        </UiButton>
      </div>

      <ul class="flex max-h-72 flex-col gap-1.5 overflow-y-auto pr-0.5">
        <li
          v-for="item in learning.pendingSuggestions"
          :key="item.id"
          class="rounded-md border border-line bg-surface-2 px-2.5 py-2"
        >
          <div class="flex items-center gap-1.5">
            <UiBadge
              :status="item.kind === 'skill' ? 'lote' : 'backlog'"
              :label="item.kind === 'skill' ? 'Skill' : 'Memória'"
            />
            <span class="min-w-0 flex-1 truncate text-caption font-semibold text-ink">
              {{ item.title }}
            </span>
          </div>
          <p class="mt-1 line-clamp-3 whitespace-pre-wrap text-label text-muted">
            {{ item.content }}
          </p>
          <p v-if="item.rationale" class="mt-1 line-clamp-2 text-meta text-muted">
            {{ item.rationale }}
          </p>
          <div class="mt-2 flex items-center gap-1.5">
            <UiButton
              variant="primary"
              size="sm"
              :disabled="learning.busyIds.has(item.id)"
              @press="learning.apply(item.id)"
            >
              Aplicar
            </UiButton>
            <UiButton
              variant="quiet"
              size="sm"
              :disabled="learning.busyIds.has(item.id)"
              @press="learning.dismiss(item.id)"
            >
              Descartar
            </UiButton>
          </div>
        </li>
      </ul>
    </div>
  </div>
</template>
