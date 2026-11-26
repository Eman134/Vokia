<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import AgentPortrait from '../AgentPortrait.vue'
import MarkdownText from '../MarkdownText.vue'
import { agentName } from '../agents'
import { useBtw } from './useBtw'

const btw = useBtw()
const active = computed(() => btw.state.active)
const copied = ref(false)

async function copyAnswer(): Promise<void> {
  const text = active.value?.answer
  if (!text) return
  try {
    await navigator.clipboard.writeText(text)
    copied.value = true
    setTimeout(() => {
      copied.value = false
    }, 1500)
  } catch {
    // ignore clipboard errors
  }
}

function onKeydown(e: KeyboardEvent): void {
  if (e.key === 'Escape' && active.value) {
    btw.dismiss()
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <aside
    v-if="active"
    data-testid="btw-window"
    class="pointer-events-auto absolute bottom-full left-1/2 mb-3 w-[min(540px,calc(100vw-2rem))] -translate-x-1/2 rounded-lg border border-line bg-surface p-3.5 shadow-paper"
    role="status"
    aria-live="polite"
  >
    <header class="flex items-center gap-2">
      <AgentPortrait :who="active.agent" :size="20" class="shrink-0" />
      <span class="rounded-sm bg-accent-soft px-1.5 py-0.5 font-mono text-tiny font-semibold text-accent">/btw</span>
      <span class="text-caption font-semibold text-ink">{{ agentName(active.agent) }}</span>
      <span
        v-if="active.status === 'thinking'"
        class="inline-flex items-center gap-1.5 text-tiny text-muted"
      >
        <span class="size-2 animate-pulse rounded-full bg-accent" aria-hidden="true" />
        pensando em segundo plano…
      </span>
      <div class="ml-auto flex items-center gap-1.5">
        <button
          v-if="active.status === 'done' && active.answer"
          type="button"
          class="cursor-pointer rounded-sm px-1.5 py-0.5 text-tiny text-muted hover:bg-hover hover:text-ink"
          @click="copyAnswer"
        >
          {{ copied ? 'Copiado' : 'Copiar' }}
        </button>
        <button
          type="button"
          class="inline-flex size-6 cursor-pointer items-center justify-center rounded-sm text-muted hover:bg-hover hover:text-ink"
          aria-label="Fechar janela /btw"
          title="Fechar (Esc)"
          @click="btw.dismiss()"
        >
          ×
        </button>
      </div>
    </header>

    <p class="mt-1.5 truncate text-meta text-muted" :title="active.question">
      {{ active.question }}
    </p>

    <div
      v-if="active.status === 'done'"
      class="mt-2 max-h-64 overflow-y-auto border-t border-line pt-2 text-small text-ink"
    >
      <MarkdownText :text="active.answer || 'Sem resposta.'" />
    </div>
    <p v-else-if="active.status === 'error'" class="mt-2 border-t border-line pt-2 text-caption text-danger">
      {{ active.error }}
    </p>
  </aside>
</template>
